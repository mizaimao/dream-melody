// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version. See <https://www.gnu.org/licenses/>.

// Serving the app and driving a headless browser, for the tests that need one.

import { createServer } from "node:http";
import { execFile } from "node:child_process";
import { readFile, access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const web = dirname(dirname(fileURLToPath(import.meta.url)));

const CHROMES = [
  process.env.CHROME,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Chromium.app/Contents/MacOS/Chromium",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
].filter(Boolean);

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".css": "text/css",
  ".json": "application/json",
  ".wasm": "application/wasm",
};

/** The first browser this machine has, or null. */
export async function findChrome() {
  for (const path of CHROMES) {
    try {
      await access(path);
      return path;
    } catch {
      /* try the next one */
    }
  }
  return null;
}

/** Serves the app on a free loopback port. */
export async function serveApp() {
  const server = createServer(async (request, response) => {
    const asked = new URL(request.url, "http://localhost").pathname;
    const path = join(web, normalize(asked === "/" ? "/index.html" : asked));
    if (!path.startsWith(web)) {
      response.writeHead(403).end();
      return;
    }
    try {
      const body = await readFile(path);
      response.writeHead(200, {
        "content-type": TYPES[extname(path)] ?? "application/octet-stream",
      });
      response.end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    origin: `http://127.0.0.1:${server.address().port}`,
    close: () => server.close(),
  };
}

/** Loads a page in a headless browser and returns its rendered DOM. */
export async function renderedDom(chrome, origin, page = "/index.html", extra = []) {
  const { stdout } = await run(
    chrome,
    [
      "--headless",
      "--disable-gpu",
      "--no-sandbox",
      "--virtual-time-budget=8000",
      "--window-size=1200,900",
      ...extra,
      "--dump-dom",
      `${origin}${page}`,
    ],
    { maxBuffer: 64 * 1024 * 1024 },
  );
  return stdout;
}

/** The markup of one element, by id, without its wrapper. */
export function insideOf(dom, id) {
  const opening = dom.indexOf(`<div id="${id}"`);
  if (opening < 0) return "";
  const from = dom.indexOf(">", opening) + 1;
  return dom.slice(from, dom.indexOf("</div>", from));
}
