import { useEffect, useState, type ComponentProps } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  ChevronDown,
  DollarSign,
  Ellipsis,
  Moon,
  Plus,
  Send,
  ShoppingCart,
  Sun,
  TrendingDown,
  TrendingUp,
  Users,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

const stats = [
  { title: "Receita", value: "R$ 45.231", change: "+20,1%", up: true, icon: DollarSign },
  { title: "Usuários ativos", value: "2.350", change: "+18,0%", up: true, icon: Users },
  { title: "Pedidos", value: "1.204", change: "-4,3%", up: false, icon: ShoppingCart },
  { title: "Conversão", value: "3,2%", change: "+0,4 p.p.", up: true, icon: Activity },
];

const orders = [
  { id: "#1042", customer: "Ana Souza", status: "Pago", total: "R$ 320,00" },
  { id: "#1041", customer: "Bruno Lima", status: "Pendente", total: "R$ 89,90" },
  { id: "#1040", customer: "Carla Dias", status: "Cancelado", total: "R$ 150,00" },
  { id: "#1039", customer: "Diego Rocha", status: "Pago", total: "R$ 1.249,00" },
] as const;

const statusVariant: Record<
  (typeof orders)[number]["status"],
  ComponentProps<typeof Badge>["variant"]
> = {
  Pago: "secondary",
  Pendente: "outline",
  Cancelado: "destructive",
};

function App() {
  const [dark, setDark] = useState(
    () => window.matchMedia("(prefers-color-scheme: dark)").matches,
  );
  const [name, setName] = useState("");
  const [greetMsg, setGreetMsg] = useState("");

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
  }, [dark]);

  async function greet() {
    try {
      // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
      setGreetMsg(await invoke<string>("greet", { name }));
    } catch {
      // `invoke` only exists inside the Tauri webview
      setGreetMsg("Fora do Tauri. Rode com `pnpm tauri dev` para chamar o Rust.");
    }
  }

  return (
    <div className="min-h-screen bg-background">
      <header className="border-b">
        <div className="mx-auto flex max-w-5xl items-center justify-between gap-4 px-6 py-4">
          <div>
            <h1 className="font-heading text-lg font-semibold">Dashboard</h1>
            <p className="text-sm text-muted-foreground">Demo do shadcn/ui + Tauri</p>
          </div>
          <div className="flex items-center gap-2 text-muted-foreground">
            <Sun className="size-4" />
            <Switch checked={dark} onCheckedChange={setDark} aria-label="Modo escuro" />
            <Moon className="size-4" />
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-5xl space-y-6 px-6 py-6">
        <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {stats.map((s) => (
            <Card key={s.title}>
              <CardHeader>
                <CardDescription>{s.title}</CardDescription>
                <CardTitle className="text-2xl tabular-nums">{s.value}</CardTitle>
                <CardAction>
                  <s.icon className="size-4 text-muted-foreground" />
                </CardAction>
              </CardHeader>
              <CardContent className="flex items-center gap-2">
                <Badge variant={s.up ? "secondary" : "destructive"}>
                  {s.up ? <TrendingUp data-icon="inline-start" /> : <TrendingDown data-icon="inline-start" />}
                  {s.change}
                </Badge>
                <span className="text-xs text-muted-foreground">vs. mês anterior</span>
              </CardContent>
            </Card>
          ))}
        </section>

        <Tabs defaultValue="components">
          <TabsList>
            <TabsTrigger value="components">Componentes</TabsTrigger>
            <TabsTrigger value="table">Tabela</TabsTrigger>
            <TabsTrigger value="tauri">Tauri</TabsTrigger>
          </TabsList>

          <TabsContent value="components" className="grid gap-4 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Formulário</CardTitle>
                <CardDescription>Input, Select, Checkbox e Switch.</CardDescription>
              </CardHeader>
              <CardContent className="grid gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="email">E-mail</Label>
                  <Input id="email" type="email" placeholder="voce@exemplo.com" />
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="plan">Plano</Label>
                  <Select defaultValue="pro">
                    <SelectTrigger id="plan" className="w-full">
                      <SelectValue placeholder="Escolha um plano" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="free">Gratuito</SelectItem>
                      <SelectItem value="pro">Pro</SelectItem>
                      <SelectItem value="enterprise">Empresarial</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div className="flex items-center gap-2">
                  <Checkbox id="terms" defaultChecked />
                  <Label htmlFor="terms">Aceito os termos de uso</Label>
                </div>
                <div className="flex items-center justify-between gap-2">
                  <Label htmlFor="notify">Notificações por e-mail</Label>
                  <Switch id="notify" defaultChecked />
                </div>
              </CardContent>
              <CardFooter className="justify-end gap-2">
                <Button variant="outline">Cancelar</Button>
                <Button>Salvar</Button>
              </CardFooter>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Botões e overlays</CardTitle>
                <CardDescription>Variantes, Dialog, Dropdown e Progress.</CardDescription>
              </CardHeader>
              <CardContent className="grid gap-4">
                <div className="flex flex-wrap gap-2">
                  <Button>Default</Button>
                  <Button variant="secondary">Secondary</Button>
                  <Button variant="outline">Outline</Button>
                  <Button variant="ghost">Ghost</Button>
                  <Button variant="destructive">Destructive</Button>
                  <Button variant="link">Link</Button>
                </div>
                <Separator />
                <div className="flex flex-wrap gap-2">
                  <Dialog>
                    <DialogTrigger asChild>
                      <Button>
                        <Plus data-icon="inline-start" />
                        Novo relatório
                      </Button>
                    </DialogTrigger>
                    <DialogContent>
                      <DialogHeader>
                        <DialogTitle>Novo relatório</DialogTitle>
                        <DialogDescription>Dê um nome para o relatório.</DialogDescription>
                      </DialogHeader>
                      <div className="grid gap-2">
                        <Label htmlFor="report-name">Nome</Label>
                        <Input id="report-name" placeholder="Vendas de outubro" />
                      </div>
                      <DialogFooter>
                        <DialogClose asChild>
                          <Button variant="outline">Cancelar</Button>
                        </DialogClose>
                        <DialogClose asChild>
                          <Button>Criar</Button>
                        </DialogClose>
                      </DialogFooter>
                    </DialogContent>
                  </Dialog>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="outline">
                        Ações
                        <ChevronDown data-icon="inline-end" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start">
                      <DropdownMenuLabel>Relatório</DropdownMenuLabel>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem>Exportar CSV</DropdownMenuItem>
                      <DropdownMenuItem>Duplicar</DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem variant="destructive">Excluir</DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
                <Separator />
                <div className="grid gap-2">
                  <div className="flex justify-between text-sm">
                    <span>Meta do mês</span>
                    <span className="text-muted-foreground tabular-nums">66%</span>
                  </div>
                  <Progress value={66} />
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="table">
            <Card>
              <CardHeader>
                <CardTitle>Pedidos recentes</CardTitle>
                <CardDescription>Table com Badge e menu por linha.</CardDescription>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Pedido</TableHead>
                      <TableHead>Cliente</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Total</TableHead>
                      <TableHead className="w-10" />
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {orders.map((o) => (
                      <TableRow key={o.id}>
                        <TableCell className="font-medium">{o.id}</TableCell>
                        <TableCell>{o.customer}</TableCell>
                        <TableCell>
                          <Badge variant={statusVariant[o.status]}>{o.status}</Badge>
                        </TableCell>
                        <TableCell className="text-right tabular-nums">{o.total}</TableCell>
                        <TableCell>
                          <DropdownMenu>
                            <DropdownMenuTrigger asChild>
                              <Button variant="ghost" size="icon-sm" aria-label="Ações do pedido">
                                <Ellipsis />
                              </Button>
                            </DropdownMenuTrigger>
                            <DropdownMenuContent align="end">
                              <DropdownMenuItem>Ver detalhes</DropdownMenuItem>
                              <DropdownMenuItem>Reenviar nota</DropdownMenuItem>
                            </DropdownMenuContent>
                          </DropdownMenu>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="tauri">
            <Card>
              <CardHeader>
                <CardTitle>Comando Rust</CardTitle>
                <CardDescription>
                  Chama o comando <code>greet</code> de <code>src-tauri/src/lib.rs</code> com{" "}
                  <code>invoke</code>.
                </CardDescription>
              </CardHeader>
              <CardContent className="grid gap-4">
                <form
                  className="flex gap-2"
                  onSubmit={(e) => {
                    e.preventDefault();
                    greet();
                  }}
                >
                  <Input
                    value={name}
                    onChange={(e) => setName(e.currentTarget.value)}
                    placeholder="Digite um nome..."
                  />
                  <Button type="submit">
                    <Send data-icon="inline-start" />
                    Enviar
                  </Button>
                </form>
                {greetMsg && <p className="text-sm text-muted-foreground">{greetMsg}</p>}
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </main>
    </div>
  );
}

export default App;
