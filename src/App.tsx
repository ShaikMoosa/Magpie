import logo from "./assets/logo.png";

export default function App() {
  return (
    <main className="grid h-full place-items-center bg-subtle">
      <div className="flex flex-col items-center gap-3 text-center">
        <img src={logo} alt="" className="size-14 rounded-2xl" />
        <h1 className="text-2xl font-semibold tracking-tight">Magpie</h1>
        <p className="text-sm text-ink-2">Project setup — the app shell lands in the next pull request.</p>
      </div>
    </main>
  );
}
