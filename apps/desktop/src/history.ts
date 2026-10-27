/**
 * 撤销 / 重做的总入口：⌘Z / ⌘⇧Z 按键和"编辑"菜单里的撤销 / 重做都走这里。
 * 焦点在文本框里时交给文本框自己（系统的文字撤销）；不然交给界面上的撤销栈（照片墙注册的那一个）。
 */
type Kind = "undo" | "redo";
type Handler = Record<Kind, () => void>;

let handler: Handler | null = null;

/** 注册界面的撤销栈；返回注销函数。同一时间只有一个（照片墙）。 */
export function setHistoryHandler(h: Handler): () => void {
  handler = h;
  return () => { if (handler === h) handler = null; };
}

const editable = (el: Element | null) => !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || (el as HTMLElement).isContentEditable);

/** 菜单的快捷键和网页里的按键有时会一起到（WebKit 先给网页、网页没拦再给菜单）：120 ms 内同一种只算一次。 */
let last: { kind: Kind; at: number } = { kind: "undo", at: -1e9 };
export function runHistory(kind: Kind) {
  const now = performance.now();
  if (last.kind === kind && now - last.at < 120) return;
  last = { kind, at: now };
  handler?.[kind]();
}

/** "编辑"菜单里的撤销 / 重做。 */
export function editHistory(kind: Kind) {
  if (editable(document.activeElement)) { document.execCommand(kind); return; }
  runHistory(kind);
}

/** 按键：⌘Z 撤销，⌘⇧Z 重做（文本框里不管，让系统处理）。返回是不是处理了。 */
export function historyKey(e: KeyboardEvent): boolean {
  if (!(e.metaKey || e.ctrlKey) || e.altKey || e.key.toLowerCase() !== "z" || editable(e.target as Element)) return false;
  e.preventDefault();
  runHistory(e.shiftKey ? "redo" : "undo");
  return true;
}
