/** 首页用到的几枚线条图标，定义一次，各处用 <Ic id="…" /> 引用。 */
export function IconDefs() {
  return (
    <svg width="0" height="0" aria-hidden className="absolute">
      <defs>
        <symbol id="i-search" viewBox="0 0 24 24"><circle cx="11" cy="11" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M16 16l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></symbol>
        <symbol id="i-shield" viewBox="0 0 24 24"><path d="M12 3.5l7 2.6v5.4c0 4.3-2.9 7.6-7 9-4.1-1.4-7-4.7-7-9V6.1l7-2.6z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" /><path d="M8.8 12.2l2.2 2.2 4.2-4.4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></symbol>
        <symbol id="i-aperture" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M14.5 3.9L9.3 13M19.9 9.5H9.6M18.1 17.6L12.9 8.6M9.5 20.1l5.2-9M4.1 14.5h10.3M5.9 6.4l5.2 9" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></symbol>
        <symbol id="i-pair" viewBox="0 0 24 24"><rect x="3" y="6" width="12" height="12" rx="2" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M9 6V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-1" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><path d="M5.5 15.5l2.6-3 2 2.2 1.6-1.7 2.3 2.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></symbol>
        <symbol id="i-xmp" viewBox="0 0 24 24"><path d="M6 3.5h7.5L19 9v10.5a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-15a1 1 0 0 1 1-1z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" /><path d="M13.5 3.5V9H19" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" /><path d="M8.5 13.5l1.5 2-1.5 2M12.5 17.5h3" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" /></symbol>
        <symbol id="i-keys" viewBox="0 0 24 24"><rect x="3" y="6" width="18" height="12" rx="2" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M7 10h.01M11 10h.01M15 10h.01M7 14h10" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" /></symbol>
        <symbol id="i-raw" viewBox="0 0 24 24"><rect x="3.5" y="4.5" width="17" height="15" rx="2" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M13 7.5l-3 5h3l-1.5 4.5 4.5-6h-3l1.5-3.5z" fill="currentColor" /></symbol>
        <symbol id="i-pace" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.5" fill="none" stroke="currentColor" strokeWidth="1.6" /><path d="M12 3.5a8.5 8.5 0 0 1 8.5 8.5" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" /><path d="M9 12.5l2 2 4-4.5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></symbol>
        <symbol id="i-star" viewBox="0 0 24 24"><path d="M12 3.6l2.6 5.4 5.9.8-4.3 4.1 1.1 5.9L12 17l-5.3 2.8 1.1-5.9-4.3-4.1 5.9-.8z" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" /></symbol>
        <symbol id="i-offline" viewBox="0 0 24 24"><path d="M3.5 9a13 13 0 0 1 17 0M6.5 12.3a8.5 8.5 0 0 1 11 0M9.6 15.6a4 4 0 0 1 4.8 0" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /><circle cx="12" cy="19" r="1.1" fill="currentColor" /><path d="M4 4l16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" /></symbol>
      </defs>
    </svg>
  );
}

export function Ic({ id }: { id: string }) {
  return <svg className="ic" aria-hidden><use href={`#i-${id}`} /></svg>;
}
