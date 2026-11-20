"use client";
import { Toaster } from "sonner";
import { SolanaWalletProvider } from "@/components/solana-wallet-provider";

export function Providers({ children }: { children: React.ReactNode }) {
  return (
    <SolanaWalletProvider>
      {children}
      <Toaster
        theme="dark"
        position="top-right"
        toastOptions={{
          style: {
            background: "var(--color-surface-2)",
            border: "1px solid var(--color-border-strong)",
            color: "var(--color-text)",
          },
        }}
      />
    </SolanaWalletProvider>
  );
}
