import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";
import { Providers } from "@/components/providers";
import { SidebarNav } from "./sidebar-nav";
import { EnvBannerClient } from "./env-banner";

const inter = Inter({
  variable: "--font-sans",
  subsets: ["latin"],
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Kalshi Bot Dashboard",
  description: "Trading bot control panel",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${inter.variable} ${jetbrainsMono.variable} dark h-full`}
    >
      <body className="min-h-full flex bg-[#0a0a0f] text-zinc-100 antialiased">
        <Providers>
          <SidebarNav />
          <main className="flex-1 overflow-auto">
            <EnvBannerClient />
            <div className="p-8">{children}</div>
          </main>
        </Providers>
      </body>
    </html>
  );
}
