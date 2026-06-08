# STF Feature-Completeness Tasklist

This document tracks the work required for `adb_client` to serve as the
device-side backend for an [STF](https://github.com/DeviceFarmer/stf)
(DeviceFarmer / Smartphone Test Farm)–style device farm.

The goal is parity with the ADB surface that STF (historically via `adbkit`)
exercises: provisioning devices, running an on-device agent, wiring up
forward/reverse sockets, and — the headline features — **live screen
mirroring** and **remote input**.

Sections are ordered by increasing complexity and dependency. The earlier
sections are mostly thin shell wrappers; the middle sections add protocol
primitives; the final section (screen streaming) depends on several of the
earlier primitives and is by far the most involved.

## Legend

- [ ] Not started
- [~] In progress
- [x] Done

## Baseline — already available

These are already implemented and need no further work for STF:

- Device discovery / tracking (`devices`, `devices_long`, `track_devices`)
- `wait_for_device`, `connect` / `disconnect` / `pair` (wifi)
- Streaming `shell`, `shell_command`, and `exec` (long-lived agent processes)
- `push` / `pull` (sync), `install` / `uninstall` (with `--user`)
- `forward` / `reverse` (+ `*_remove` / `*_remove_all`)
- `logcat` (`get_logs`)
- `reboot`, `root`, `remount`, `tcpip`, `usb`, verity toggles
- `host_features`
- One-shot `framebuffer` / `framebuffer_bytes` (raw screen capture → PNG)
- `list_packages` (`pm list packages` wrapper)

---

## 1. Device properties & introspection

**Why:** STF needs `ro.product.*`, SDK level, ABI, and display size/density to
pick the correct architecture-specific `minicap`/`minitouch` binary and to
compute the screen projection. Today this is only reachable as raw text via
`shell_command`.

**Tasks:**

- [ ] Add `get_properties() -> HashMap<String, String>` that runs `getprop` and
      parses the `[key]: [value]` output.
- [ ] Add a typed `DeviceProperties` convenience (model, brand, manufacturer,
      `sdk` level, `release`, primary ABI, ABI list).
- [ ] Add `get_property(name) -> Option<String>` (`getprop <name>`).
- [ ] Add display info helper (`wm size`, `wm density`) → `{ width, height,
      density }`.

**Proposed API (on `ADBDeviceExt`, default methods over `shell_command`):**

```rust
fn get_properties(&mut self) -> Result<HashMap<String, String>>;
fn get_property(&mut self, name: &str) -> Result<Option<String>>;
```

**Files:** `adb_client/src/adb_device_ext.rs`, new `models/device_properties.rs`.

**Acceptance:** `get_properties()` returns a non-empty map on a real device;
unit test the `getprop` parser against captured sample output.

---

## 2. Input injection helpers

**Why:** Convenience wrappers for scripted control and as a fallback when
`minitouch` is unavailable. STF's primary path is `minitouch` (Section 6), but
typed `input` helpers are broadly useful and cheap.

**Tasks:**

- [ ] `input_keyevent(code)` → `input keyevent <code>`.
- [ ] `input_text(text)` → `input text <escaped>` (handle spaces/special chars).
- [ ] `input_tap(x, y)` → `input tap x y`.
- [ ] `input_swipe(x1, y1, x2, y2, duration_ms)` → `input swipe ...`.
- [ ] Optional: a `KeyCode` enum for the common Android keycodes.

**Files:** `adb_client/src/adb_device_ext.rs` (default methods).

**Acceptance:** each helper builds the expected command string (unit-tested);
manual smoke test on a device.

---

## 3. Screen capture convenience (`screencap`)

**Why:** Complements the existing raw `framebuffer`. `screencap -p` returns a
PNG directly from the device and is the simplest "grab current screen" path;
useful for thumbnails and as a non-streaming fallback.

**Tasks:**

- [ ] `screencap() -> Vec<u8>` running `exec:screencap -p` and collecting stdout.
- [ ] Document the trade-off vs. `framebuffer` (compressed PNG vs. raw pixels)
      in the method docs.
- [ ] Note: `screencap` over `shell:` mangles bytes via CRLF translation on some
      devices — use `exec:` (no PTY translation) to keep the PNG intact.

**Files:** `adb_client/src/adb_device_ext.rs`.

**Acceptance:** returned bytes decode as a valid PNG of the device resolution.

---

## 4. Port forwarding completeness

**Why:** STF allocates ephemeral ports with `adb forward tcp:0 localabstract:…`
and reads back the chosen port. The current `forward()` discards the server's
reply, and there is no way to enumerate existing rules.

**Tasks:**

- [ ] Change `forward` to parse and **return the allocated local port** when the
      local spec is `tcp:0` (the adb server replies with the port number).
      Keep a non-breaking shape, e.g. return `Result<Option<u16>>` or add a
      `forward_tcp(remote) -> Result<u16>` helper.
- [ ] Add `list_forward() -> Vec<ForwardRule>` (`host:list-forward`).
- [ ] Add `list_reverse() -> Vec<ForwardRule>` (`reverse:list-forward`).
- [ ] Define a `ForwardRule { serial, local, remote }` model and parse the
      newline/space-delimited server output.
- [ ] Audit `forward`/`reverse` argument naming/order (`remote` vs `local`) for
      clarity; document which side is which.

**Files:** `adb_client/src/server_device/commands/forward.rs`,
`reverse.rs`, new `models/forward_rule.rs`.

**Acceptance:** `forward_tcp` against `tcp:0` returns a usable port that a
local `TcpStream` can connect through; `list_forward` round-trips a rule that
was just created.

---

## 5. Generic local socket access (`open_local`)

**Why:** This is the foundational primitive for Section 6 and for any direct
talk to an on-device socket. STF reads `minicap` frames and writes `minitouch`
events by opening device-side sockets *through the adb server* and getting back
a raw bidirectional stream — `adbkit`'s `openLocal('localabstract:…')`. Today
the only stream-opening services exposed are `shell:`, `exec:`, and `sync:`.

**Tasks:**

- [ ] Add `ADBLocalCommand::Open(String)` (or similar) that serializes an
      arbitrary local service string and opens a stream.
- [ ] Expose `open_local(service: &str) -> impl Read + Write` on
      `ADBServerDevice`, returning an owned duplex over the transport (after
      `host:transport:<serial>`). Support at least:
      `localabstract:`, `localreserved:`, `localfilesystem:`, `tcp:`, `jdwp:`.
- [ ] Ensure the returned handle is `Send` and splittable (clone/try_clone) so a
      caller can read and write concurrently (minitouch needs both).
- [ ] Decide ownership model: hand back the raw connection vs. a thin wrapper.
- [ ] (Optional) `list_jdwp()` / `track_jdwp` for completeness.

**Proposed API:**

```rust
impl ADBServerDevice {
    /// Open a stream to a device-side local service through the adb server,
    /// e.g. `localabstract:minicap`, `tcp:5555`, `jdwp:1234`.
    pub fn open_local(&mut self, service: &str) -> Result<impl Read + Write + Send>;
}
```

**Files:** new `adb_client/src/server_device/commands/open_local.rs`,
`models/adb_local_command.rs`, `server_device/adb_server_device.rs`.

**Acceptance:** `open_local("localabstract:<name>")` against a known on-device
listener (e.g. a test `nc -l` over an abstract socket) reads/writes bytes
end-to-end; concurrent read+write from two handles works.

---

## 6. Screen streaming (minicap) — complex, do last

**Why:** This is STF's reason for existing — a live, low-latency, compressed
screen feed plus remote control. It is intentionally the last section because
it composes several earlier pieces (`push`, `exec`, `open_local`, device
properties) and carries the most moving parts.

> **Scope note:** `minicap`/`minitouch` are prebuilt native binaries shipped by
> STF. This task is about *driving* them, not reimplementing screen capture in
> Rust. Reimplementing capture (SurfaceFlinger / `scrcpy`-style encode) is a
> separate, much larger effort and explicitly out of scope here.

### 6a. Binary provisioning

- [ ] Resolve device ABI + SDK (Section 1) to select the right
      `minicap`/`minicap.so` and `minitouch` build.
- [ ] `push` the binaries to `/data/local/tmp/` and confirm executability
      (current `push` sets mode `0777`, so this should work — verify).
- [ ] Decide how the consumer supplies the binaries (path/embedded/downloaded);
      the library should accept a stream, not bundle binaries.

### 6b. minicap (screen → frames)

- [ ] Start minicap via `exec`/`shell` with the projection args
      (`-P <real_w>x<real_h>@<virt_w>x<virt_h>/<rotation>`).
- [ ] Wait for it to bind `localabstract:minicap`, then `open_local` it
      (Section 5).
- [ ] Parse the minicap **global header** (version, header size, pid, real/virt
      dimensions, orientation, quirk flags).
- [ ] Parse the per-frame framing: 4-byte little-endian length prefix + JPEG
      payload; emit frames to the caller (callback / channel / iterator).
- [ ] Handle rotation changes and minicap restarts (re-launch + re-open).
- [ ] Define a `ScreenFrame { width, height, jpeg: Vec<u8> }` type and a
      streaming API (e.g. `fn stream_screen(...) -> Receiver<ScreenFrame>`).

### 6c. minitouch (remote input)

- [ ] Start minitouch via `exec`/`shell`; `open_local("localabstract:minitouch")`.
- [ ] Parse the startup banner (`v <version>`, `^ <max_contacts> <max_x>
      <max_y> <max_pressure>`, `$ <pid>`).
- [ ] Implement the command protocol: `d` (down), `m` (move), `u` (up),
      `c` (commit), `w` (wait), with multi-touch contact slots.
- [ ] Map normalized/display coordinates to minitouch's coordinate space.

### 6d. Lifecycle & robustness

- [ ] Tie minicap/minitouch process lifetimes to the session; clean shutdown
      and `forward`/`reverse` rule cleanup.
- [ ] Reconnect/retry on socket drop and on screen-size/rotation changes.
- [ ] Backpressure / frame-dropping strategy for slow consumers.

**Files:** new module, e.g. `adb_client/src/stream/{minicap.rs, minitouch.rs}`,
behind a feature flag (e.g. `screen-stream`) to keep the dependency/feature
surface opt-in.

**Acceptance:** with binaries provided, `stream_screen` yields decodable JPEG
frames at the device resolution and survives a rotation; `minitouch` taps land
at the expected coordinates on screen.

---

## Suggested order of execution

1. Section 1 (properties) — needed by Section 6a anyway, low risk.
2. Sections 2 & 3 (input + screencap helpers) — quick wins, independent.
3. Section 4 (forward port + list) — unblocks the streaming fallback path.
4. Section 5 (`open_local`) — the key protocol primitive.
5. Section 6 (minicap/minitouch) — built on top of 1, 5 (and 4 as fallback).
