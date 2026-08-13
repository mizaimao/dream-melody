// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
// for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Dream Melody as a desktop application.
//!
//! Thin, and meant to stay thin. The interface is the same one the browser
//! runs, and the music is the same core compiled to wasm inside it — this crate
//! owns a window and nothing else. Everything worth testing lives on the other
//! side of that boundary, where it can be tested without a window.
//!
//! The interface is embedded in the executable at build time, so the
//! application is a single file that needs no server and no network.

// A release build should not raise a console window behind the application on
// Windows. It has no effect elsewhere.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// What the interface says about itself once it has drawn a score.
///
/// The interface runs in a webview, where a failure would otherwise be visible
/// only to whoever opens the developer tools. This puts it where the person who
/// launched the application will see it.
#[tauri::command]
fn interface_ready(summary: String, problems: Vec<String>) {
    println!("interface: {summary}");
    for problem in problems {
        eprintln!("problem: {problem}");
    }
}

// A window that cannot open has nowhere to report it: there is no window to
// report it in, and no caller to hand an error back to.
#[allow(clippy::expect_used)]
fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![interface_ready])
        .run(tauri::generate_context!())
        .expect("Dream Melody could not start");
}
