const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

async function download(url, dest) {
  return new Promise((resolve, reject) => {
    https.get(url, { headers: { "User-Agent": "curl/8.0" } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        return resolve(download(res.headers.location, dest));
      }
      if (res.statusCode !== 200) {
        return reject(new Error(`Failed to download ${url}: HTTP ${res.statusCode}`));
      }
      const file = fs.createWriteStream(dest);
      res.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", (err) => {
        fs.unlink(dest, () => reject(err));
      });
    }).on("error", reject);
  });
}

async function main() {
  const version = process.env.ZIG_VERSION || "0.15.2";
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  const platform = process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux";
  const ext = platform === "windows" ? "zip" : "tar.xz";
  const filename = `zig-${arch}-${platform}-${version}.${ext}`;
  const url = `https://ziglang.org/download/${version}/${filename}`;

  const zigDir = path.resolve(process.env.RUNNER_TEMP || path.join(__dirname, "..", "node_modules", ".zig-bin"));
  fs.mkdirSync(zigDir, { recursive: true });
  const archivePath = path.join(zigDir, filename);

  console.log(`Downloading Zig from ${url}...`);
  await download(url, archivePath);

  console.log(`Extracting ${filename}...`);
  execSync(`tar -xf "${archivePath}" -C "${zigDir}"`, { stdio: "inherit" });

  const extractedFolder = path.join(zigDir, `zig-${arch}-${platform}-${version}`);
  const zigExe = path.join(extractedFolder, platform === "windows" ? "zig.exe" : "zig");

  if (!fs.existsSync(zigExe)) {
    throw new Error(`Zig binary not found at ${zigExe}`);
  }

  const verOut = execSync(`"${zigExe}" version`, { encoding: "utf8" }).trim();
  console.log(`Zig ${verOut} successfully installed at ${zigExe}`);

  if (process.env.GITHUB_PATH) {
    fs.appendFileSync(process.env.GITHUB_PATH, `${extractedFolder}\n`);
  }
  if (process.env.GITHUB_ENV) {
    fs.appendFileSync(process.env.GITHUB_ENV, `ZIG=${zigExe}\n`);
  }
}

main().catch((err) => {
  console.error("Setup Zig failed:", err);
  process.exit(1);
});
