import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import { Providers } from "@/components/providers";
import { SidebarNav } from "./sidebar-nav";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
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
      className={`${geistSans.variable} ${geistMono.variable} h-full antialiased`}
    >
      <body className="min-h-full flex">
        <Providers>
          <SidebarNav />
          <main className="flex-1 overflow-auto">
            <EnvBanner />
            <div className="p-6">{children}</div>
          </main>
        </Providers>
      </body>
    </html>
  );
}

function EnvBanner() {
  return <EnvBannerClient />;
}

import { EnvBannerClient } from "./env-banner";
