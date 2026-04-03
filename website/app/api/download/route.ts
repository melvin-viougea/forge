import { NextResponse } from "next/server";

const GITHUB_REPO = "melvin-viougea/forge";

export async function GET() {
  try {
    const res = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`,
      {
        headers: { Accept: "application/vnd.github.v3+json" },
        next: { revalidate: 300 }, // cache 5 min
      }
    );

    if (!res.ok) {
      return NextResponse.redirect(
        `https://github.com/${GITHUB_REPO}/releases/latest`
      );
    }

    const release = await res.json();

    // Find the .dmg asset
    const dmgAsset = release.assets?.find((a: { name: string }) =>
      a.name.endsWith(".dmg")
    );

    if (dmgAsset) {
      return NextResponse.redirect(dmgAsset.browser_download_url);
    }

    // Fallback to release page
    return NextResponse.redirect(release.html_url);
  } catch {
    return NextResponse.redirect(
      `https://github.com/${GITHUB_REPO}/releases/latest`
    );
  }
}
