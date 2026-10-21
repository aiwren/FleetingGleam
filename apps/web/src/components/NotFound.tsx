import { Link } from "@tanstack/react-router";

/** Shown for any address that is not a page: both languages, one way back. */
export default function NotFound() {
  return (
    <main className="mx-auto flex min-h-[100dvh] max-w-6xl flex-col items-start justify-center px-5">
      <p className="font-mono text-sm text-ash">404</p>
      <h1 className="mt-3 text-[clamp(2rem,5vw,3.5rem)] font-semibold leading-[1.06] tracking-[-0.04em]">页面不存在 · Page not found</h1>
      <p className="mt-5 max-w-[48ch] text-lg text-ash">这个地址没有内容。 / There is nothing at this address.</p>
      <div className="mt-8 flex flex-wrap gap-3">
        <Link to="/" className="inline-flex min-h-12 items-center rounded-full bg-ink px-6 py-3 font-medium text-on-ink">English home</Link>
        <Link to={"/zh/" as never} className="inline-flex min-h-12 items-center rounded-full border border-line px-6 py-3 font-medium">中文首页</Link>
      </div>
    </main>
  );
}
