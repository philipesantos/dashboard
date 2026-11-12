//! The compile process on Windows (`specs/010-plugin-installation/research.md`, R2, R13): created
//! by the parent itself, since the standard library cannot yet pass the attribute that puts a
//! process in an AppContainer.
//!
//! - An AppContainer with no capabilities: no network, none of the user's files, no keychain
//!   (Credential Manager), no other process. The executable file alone is granted read and
//!   execute to it, so it can start from a per-user installation; nothing else of its folder.
//! - Only its three pipes are inherited (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`).
//! - A job object: the memory and CPU time limits, one process, no desktop, clipboard or global
//!   atoms, and death when the parent's handle closes. The process is created suspended, put in
//!   the job, and only then resumed, so it runs nothing outside it.
//! - A below-normal priority, and an environment of only what the parent passes.
//!
//! Past its CPU time, a job's process ends with `ERROR_NOT_ENOUGH_QUOTA`; past its memory, an
//! allocation fails and the child aborts ([`ABORT_EXIT_CODE`]) or ends with its own code for it.
//!
//! Part of the security core (constitution v2.7.0, Principle IX). Checked on Windows by CI only.

use std::ffi::{OsStr, OsString, c_void};
use std::fs::File;
use std::io::{Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GENERIC_EXECUTE, GENERIC_READ, HANDLE, HANDLE_FLAG_INHERIT,
    LocalFree, SetHandleInformation, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, NO_MULTIPLE_TRUSTEE, SE_FILE_OBJECT,
    SetEntriesInAclW, SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, FreeSid, NO_INHERITANCE, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME, JOB_OBJECT_UILIMIT_DESKTOP,
    JOB_OBJECT_UILIMIT_DISPLAYSETTINGS, JOB_OBJECT_UILIMIT_EXITWINDOWS,
    JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES, JOB_OBJECT_UILIMIT_READCLIPBOARD,
    JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS, JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    CreateProcessW, DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT,
    GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    PROCESS_INFORMATION, ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

use super::{CompileLimits, Ended, Running};

/// The exit code a job gives a process past its CPU time limit: `ERROR_NOT_ENOUGH_QUOTA`.
pub(super) const TIME_LIMIT_EXIT_CODE: i32 = 1816;

/// The status a process ends with when it aborts, as the standard library does when an
/// allocation fails (`__fastfail`, `STATUS_STACK_BUFFER_OVERRUN`): under the job's memory limit,
/// what an allocation past it leads to.
pub(super) const ABORT_EXIT_CODE: i32 = 0xC000_0409_u32 as i32;

/// The AppContainer the compile process runs in.
const CONTAINER: &str = "dev.philsantos.dashboard.compile";

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

fn last_error(what: &str) -> std::io::Error {
    let error = std::io::Error::last_os_error();
    std::io::Error::new(error.kind(), format!("{what}: {error}"))
}

/// A handle closed when dropped.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: a handle this module opened and nothing else closes
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// The AppContainer's SID, freed when dropped.
struct ContainerSid(PSID);

impl Drop for ContainerSid {
    fn drop(&mut self) {
        // SAFETY: a SID the system allocated for this module
        unsafe { FreeSid(self.0) };
    }
}

/// The AppContainer's SID, creating its profile the first time.
fn container() -> std::io::Result<ContainerSid> {
    let name = wide(OsStr::new(CONTAINER));
    let display = wide(OsStr::new("Dashboard compile process"));
    let mut sid: PSID = null_mut();
    // SAFETY: nul-terminated names; the SID is the system's, freed by ContainerSid
    let created = unsafe {
        CreateAppContainerProfile(
            name.as_ptr(),
            display.as_ptr(),
            display.as_ptr(),
            null(),
            0,
            &mut sid,
        )
    };
    if created >= 0 {
        return Ok(ContainerSid(sid));
    }
    let already = (0x8007_0000u32 | ERROR_ALREADY_EXISTS) as i32;
    if created != already {
        return Err(std::io::Error::other(format!(
            "CreateAppContainerProfile: HRESULT {created:#x}"
        )));
    }
    // SAFETY: as above
    let derived = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) };
    if derived < 0 {
        return Err(std::io::Error::other(format!(
            "DeriveAppContainerSidFromAppContainerName: HRESULT {derived:#x}"
        )));
    }
    Ok(ContainerSid(sid))
}

/// Grants the AppContainer read and execute on the executable file, and on nothing else: not its
/// folder, whose other files (a portable installation's data among them) stay out of its reach
/// (the review of task T053). Best effort: a file under Program Files is readable by every
/// AppContainer already, and may refuse a change.
fn grant(executable: &Path, sid: &ContainerSid) {
    let path = wide(executable.as_os_str());
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: a nul-terminated path; the descriptor the system returns is freed with LocalFree
    let read = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if read != 0 {
        return;
    }
    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: GENERIC_READ | GENERIC_EXECUTE,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_UNKNOWN,
            ptstrName: sid.0.cast(),
        },
    };
    let mut updated: *mut ACL = null_mut();
    // SAFETY: one entry; the new ACL is the system's, freed with LocalFree
    unsafe {
        if SetEntriesInAclW(1, &access, dacl, &mut updated) == 0 {
            SetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                updated,
                null(),
            );
            LocalFree(updated.cast());
        }
        LocalFree(descriptor);
    }
}

/// A pipe: the child's end inheritable, the parent's not.
fn pipe(child_reads: bool) -> std::io::Result<(Owned, Owned)> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write): (HANDLE, HANDLE) = (null_mut(), null_mut());
    // SAFETY: two out-handles the system fills
    if unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) } == 0 {
        return Err(last_error("CreatePipe"));
    }
    let (read, write) = (Owned(read), Owned(write));
    let (child, parent) = if child_reads {
        (read, write)
    } else {
        (write, read)
    };
    // SAFETY: a handle just created
    if unsafe { SetHandleInformation(parent.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(last_error("SetHandleInformation"));
    }
    Ok((child, parent))
}

/// The command line, each argument quoted as the C runtime reads it.
fn command_line(executable: &Path, arguments: &[String]) -> Vec<u16> {
    let mut line = OsString::from("\"");
    line.push(executable.as_os_str());
    line.push("\"");
    for argument in arguments {
        line.push(" ");
        line.push(argument);
    }
    wide(&line)
}

/// The environment block: `name=value` pairs, each nul-terminated, sorted, then a nul.
fn environment_block(environment: &[(String, OsString)]) -> Vec<u16> {
    let mut sorted: Vec<&(String, OsString)> = environment.iter().collect();
    sorted.sort_by_key(|(name, _)| name.to_uppercase());
    let mut block = Vec::new();
    for (name, value) in sorted {
        let mut pair = OsString::from(name);
        pair.push("=");
        pair.push(value);
        block.extend(pair.encode_wide());
        block.push(0);
    }
    block.push(0);
    block
}

/// A job with the compile limits.
fn job(limits: &CompileLimits) -> std::io::Result<Owned> {
    // SAFETY: an unnamed job with default security
    let job = Owned(unsafe { CreateJobObjectW(null(), null()) });
    if job.0.is_null() {
        return Err(last_error("CreateJobObjectW"));
    }
    // SAFETY: a zeroed plain struct, then filled
    let mut extended: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    extended.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    // In 100-nanosecond units
    extended.BasicLimitInformation.PerProcessUserTimeLimit =
        i64::try_from(limits.cpu_time.as_nanos() / 100).unwrap_or(i64::MAX);
    extended.BasicLimitInformation.ActiveProcessLimit = 1;
    extended.ProcessMemoryLimit = usize::try_from(limits.memory_bytes).unwrap_or(usize::MAX);
    let interface = JOBOBJECT_BASIC_UI_RESTRICTIONS {
        UIRestrictionsClass: JOB_OBJECT_UILIMIT_DESKTOP
            | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
            | JOB_OBJECT_UILIMIT_EXITWINDOWS
            | JOB_OBJECT_UILIMIT_GLOBALATOMS
            | JOB_OBJECT_UILIMIT_HANDLES
            | JOB_OBJECT_UILIMIT_READCLIPBOARD
            | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
            | JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
    };
    // SAFETY: each struct is the one its information class names, with its size
    unsafe {
        if SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const extended).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            return Err(last_error("SetInformationJobObject (limits)"));
        }
        if SetInformationJobObject(
            job.0,
            JobObjectBasicUIRestrictions,
            (&raw const interface).cast(),
            size_of::<JOBOBJECT_BASIC_UI_RESTRICTIONS>() as u32,
        ) == 0
        {
            return Err(last_error("SetInformationJobObject (interface)"));
        }
    }
    Ok(job)
}

/// The attribute list that puts the process in the AppContainer and lets it inherit only
/// `inherited`; deleted when dropped.
struct Attributes {
    buffer: Vec<u8>,
}

impl Attributes {
    fn new(capabilities: &SECURITY_CAPABILITIES, inherited: &[HANDLE; 3]) -> std::io::Result<Self> {
        let mut size = 0usize;
        // SAFETY: the first call only reports the size it needs
        unsafe { InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut size) };
        let mut buffer = vec![0u8; size];
        let list = buffer.as_mut_ptr().cast::<c_void>();
        // SAFETY: a buffer of the size the system asked for; the values outlive the list, which
        // is used only by the CreateProcessW call within spawn
        unsafe {
            if InitializeProcThreadAttributeList(list, 2, 0, &mut size) == 0 {
                return Err(last_error("InitializeProcThreadAttributeList"));
            }
            let attributes = Attributes { buffer };
            let list = attributes.list();
            if UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                (capabilities as *const SECURITY_CAPABILITIES).cast(),
                size_of::<SECURITY_CAPABILITIES>(),
                null_mut(),
                null(),
            ) == 0
            {
                return Err(last_error("UpdateProcThreadAttribute (AppContainer)"));
            }
            if UpdateProcThreadAttribute(
                list,
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                inherited.as_ptr().cast(),
                size_of_val(inherited),
                null_mut(),
                null(),
            ) == 0
            {
                return Err(last_error("UpdateProcThreadAttribute (handles)"));
            }
            Ok(attributes)
        }
    }

    fn list(&self) -> *mut c_void {
        self.buffer.as_ptr().cast_mut().cast()
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        // SAFETY: a list this module initialized
        unsafe { DeleteProcThreadAttributeList(self.list()) };
    }
}

/// The compile process, as the parent drives it: its handle, its job, and its pipes.
struct Child {
    process: Owned,
    /// Closing it kills the process (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
    _job: Owned,
    stdin: Option<File>,
    stdout: Option<File>,
    stderr: Option<File>,
}

impl Running for Child {
    fn stdin(&mut self) -> Option<Box<dyn Write + Send>> {
        self.stdin
            .take()
            .map(|file| Box::new(file) as Box<dyn Write + Send>)
    }

    fn stdout(&mut self) -> Option<Box<dyn Read + Send>> {
        self.stdout
            .take()
            .map(|file| Box::new(file) as Box<dyn Read + Send>)
    }

    fn stderr(&mut self) -> Option<Box<dyn Read + Send>> {
        self.stderr
            .take()
            .map(|file| Box::new(file) as Box<dyn Read + Send>)
    }

    fn try_wait(&mut self) -> std::io::Result<Option<Ended>> {
        // SAFETY: the process handle this child owns
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_OBJECT_0 {
            return Ok(None);
        }
        let mut code = 0u32;
        // SAFETY: as above
        if unsafe { GetExitCodeProcess(self.process.0, &mut code) } == 0 {
            return Err(last_error("GetExitCodeProcess"));
        }
        Ok(Some(Ended {
            success: code == 0,
            code: Some(code as i32),
            signal: None,
            shown: format!("exit code {code:#x}"),
        }))
    }

    fn kill(&mut self) {
        // SAFETY: as above
        unsafe { TerminateProcess(self.process.0, 1) };
    }

    fn wait(&mut self) {
        // SAFETY: as above
        unsafe { WaitForSingleObject(self.process.0, INFINITE) };
    }
}

/// Starts `executable` with `arguments` and `environment` alone, in the AppContainer and the job.
pub(super) fn spawn(
    executable: &Path,
    arguments: &[String],
    environment: &[(String, OsString)],
    limits: &CompileLimits,
) -> std::io::Result<Box<dyn Running>> {
    let sid = container()?;
    grant(executable, &sid);
    let (child_in, parent_in) = pipe(true)?;
    let (child_out, parent_out) = pipe(false)?;
    let (child_err, parent_err) = pipe(false)?;
    let capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: sid.0,
        Capabilities: null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    let inherited = [child_in.0, child_out.0, child_err.0];
    let attributes = Attributes::new(&capabilities, &inherited)?;
    let job = job(limits)?;

    // SAFETY: a zeroed plain struct, then filled
    let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = child_in.0;
    startup.StartupInfo.hStdOutput = child_out.0;
    startup.StartupInfo.hStdError = child_err.0;
    startup.lpAttributeList = attributes.list();
    let application = wide(executable.as_os_str());
    let mut line = command_line(executable, arguments);
    let block = environment_block(environment);
    // SAFETY: a zeroed plain struct the call fills
    let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: every pointer is to a value alive for the call; the command line is writable, as
    // CreateProcessW requires
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_UNICODE_ENVIRONMENT
                | CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | BELOW_NORMAL_PRIORITY_CLASS,
            block.as_ptr().cast(),
            null(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    if created == 0 {
        // Read before anything else can change the thread's last error
        let error = std::io::Error::last_os_error();
        return Err(std::io::Error::new(
            error.kind(),
            format!(
                "CreateProcessW in the AppContainer, for {}: {error}",
                executable.display()
            ),
        ));
    }
    let handle = Owned(process.hProcess);
    let thread = Owned(process.hThread);
    // SAFETY: the process handle just created; resumed only once it is in the job
    unsafe {
        if AssignProcessToJobObject(job.0, handle.0) == 0 {
            let error = last_error("AssignProcessToJobObject");
            TerminateProcess(handle.0, 1);
            return Err(error);
        }
        if ResumeThread(thread.0) == u32::MAX {
            let error = last_error("ResumeThread");
            TerminateProcess(handle.0, 1);
            return Err(error);
        }
    }
    drop((child_in, child_out, child_err, attributes, thread));
    // SAFETY: each parent end is a handle this module owns, handed to a File that closes it
    let file = |owned: Owned| {
        let handle = owned.0;
        std::mem::forget(owned);
        unsafe { File::from_raw_handle(handle) }
    };
    Ok(Box::new(Child {
        process: handle,
        _job: job,
        stdin: Some(file(parent_in)),
        stdout: Some(file(parent_out)),
        stderr: Some(file(parent_err)),
    }))
}
