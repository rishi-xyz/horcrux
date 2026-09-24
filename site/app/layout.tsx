import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  weight: ["400", "500", "600", "700"],
  variable: "--font-jetbrains-mono",
  display: "swap",
});

export const metadata: Metadata = {
  title: "HORCRUX — Offline Threshold Key Management",
  description:
    "HORCRUX splits a blockchain private key across guardians with Shamir's Secret Sharing and FROST threshold MPC. No single point of failure, no internet required to sign.",
  icons: {
    icon: [
      {
        url:
          "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'%3E%3Crect width='32' height='32' rx='7' fill='%230b0f16'/%3E%3Cpath d='M16 6l8 4v6c0 6-3.6 9.6-8 10-4.4-.4-8-4-8-10v-6l8-4z' fill='none' stroke='%234ee1c2' stroke-width='2'/%3E%3Ccircle cx='16' cy='15' r='2.4' fill='%234ee1c2'/%3E%3Cpath d='M16 17.4V22' stroke='%234ee1c2' stroke-width='2' stroke-linecap='round'/%3E%3C/svg%3E",
      },
    ],
  },
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${inter.variable} ${jetbrainsMono.variable}`}>
      <body className="bg-bg text-text font-sans antialiased">{children}</body>
    </html>
  );
}
