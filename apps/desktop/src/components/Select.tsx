import { SelectHTMLAttributes } from "react";

/** A native <select> (keeps keyboard and OS behaviour, scales to long lists) restyled to match the app. */
export default function Select({ className = "", ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <span className={`select ${className}`}>
      <select {...props} />
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d="M2.5 3.8L5 6.3l2.5-2.5" /></svg>
    </span>
  );
}
