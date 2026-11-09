-- The accessibility tree of a host window through the AX API, for host/tests/window.rs.
-- Usage: osascript ax.applescript dump PID TITLE | find PID TITLE NAME
on run argv
	set command to item 1 of argv
	set pid to (item 2 of argv) as integer
	set title to item 3 of argv
	set out to ""
	tell application "System Events"
		if command is "dump" then
			set out to "# UI scripting enabled: " & (UI elements enabled as text) & linefeed
		end if
		set proc to first process whose unix id is pid
		if command is "dump" then
			set out to out & "# windows: " & ((name of every window of proc) as text) & linefeed
		end if
		set win to (first window of proc whose name is title)
		set elements to entire contents of win
		if command is "dump" then
			set out to out & "# elements: " & ((count of elements) as text) & linefeed
		end if
		repeat with element in elements
			set label to ""
			try
				set label to (name of element) as text
			end try
			if label is "" or label is "missing value" then
				try
					set label to (description of element) as text
				end try
			end if
			if command is "dump" then
				set out to out & (role of element) & tab & label & linefeed
			else if command is "find" and label is (item 4 of argv) then
				set p to position of element
				set s to size of element
				return ((item 1 of p) as text) & " " & ((item 2 of p) as text) & " " & ((item 1 of s) as text) & " " & ((item 2 of s) as text)
			end if
		end repeat
	end tell
	return out
end run
