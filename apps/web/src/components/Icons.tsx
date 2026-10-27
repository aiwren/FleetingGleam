/** 新设计稿里用到的几枚线条图标，定义一次，各处用 <Ic id="…" /> 引用。 */
export function IconDefs() {
  return (
    <svg width="0" height="0" aria-hidden className="absolute">
      <defs>
        <symbol id="i-check" viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" /></symbol>
        <symbol id="i-x" viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" /></symbol>
        <symbol id="i-search" viewBox="0 0 24 24"><circle cx="11" cy="11" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M16 16l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></symbol>
        <symbol id="i-shield" viewBox="0 0 24 24"><path d="M12 3.5l7 2.6v5.4c0 4.3-2.9 7.6-7 9-4.1-1.4-7-4.7-7-9V6.1l7-2.6z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" /><path d="M8.8 12.2l2.2 2.2 4.2-4.4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></symbol>
        <symbol id="i-aperture" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M14.5 3.9L9.3 13M19.9 9.5H9.6M18.1 17.6L12.9 8.6M9.5 20.1l5.2-9M4.1 14.5h10.3M5.9 6.4l5.2 9" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></symbol>
        <symbol id="i-offline" viewBox="0 0 24 24"><path d="M3.5 9a13 13 0 0 1 17 0M6.5 12.3a8.5 8.5 0 0 1 11 0M9.6 15.6a4 4 0 0 1 4.8 0" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><circle cx="12" cy="19" r="1.1" fill="currentColor" /><path d="M4 4l16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></symbol>
        <symbol id="i-folder" viewBox="0 0 24 24"><path d="M3.5 7.5a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" /></symbol>
        <symbol id="i-trash" viewBox="0 0 24 24"><path d="M4.5 7h15M9.5 7V5h5v2M6.5 7l1 12.5h9l1-12.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></symbol>
      </defs>
    </svg>
  );
}

export function Ic({ id }: { id: string }) {
  return <svg className="ic" aria-hidden><use href={`#i-${id}`} /></svg>;
}

/** 保留标记：琥珀色圆底加一个勾（琥珀色只用来表示"保留"）。 */
export function Badge({ label }: { label: string }) {
  return <span className="badge"><Ic id="check" /><span className="sr-only">{label}</span></span>;
}
