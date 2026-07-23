# Quota Display Hardware: Pi Zero 2 W vs ESP32 — Recommendation

**Date:** 2026-07-20

## Recommendation (lead)

**ESP32 all-in-one board with integrated display, running ESPHome, polling a tiny JSON endpoint on the Mac.**

Concrete pick: **LilyGO T-Display-S3** (~$23, 1.9" IPS 170x320 color TFT, ESP32-S3, USB-C, no soldering) + ESPHome `http_request` + `json` components polling a `python -m http.server`-style JSON endpoint VibeProxy exposes locally (e.g. `http://<mac-ip>:PORT/quota.json`), rendered as text % + a `lambda`-drawn bar via ESPHome's `display` component.

**Why this wins for this exact use case (3 reasons):**
1. **Task fits the hardware, not the other way round.** This is a 2-value, 1-2 min poll, static layout gadget — a $15-25 microcontroller with an integrated screen does this natively; a full Linux SBC is overkill and adds failure surface (SD card, OS updates, package rot) for zero benefit here.
2. **All-in-one boards eliminate the wiring/soldering risk.** T-Display-S3 / M5Stack units are single-board, USB-C powered, screen pre-attached — closest to "buy it, flash it, done" for a dev who's less confident with electronics.
3. **Reliability profile matches "leave it on for weeks."** No SD card to corrupt, no filesystem to fsck, near-instant reboot on power blip, ESPHome ships OTA + watchdog + WiFi-reconnect handling out of the box — less to babysit than a headless Pi.

**When Pi Zero 2 W is the better call instead:** if the user wants to skip Home-Assistant/ESPHome entirely and just SSH in + write raw Python + curl, or wants a browser-kiosk dashboard with richer UI (multiple charts, fonts, animations) later, or already runs Home Assistant and wants this to be one more HA-integrated node with more compute headroom for future features (e.g. also scraping other menubar data, running a local web UI). Also better if Pi Zero 2 W stock/price is acceptable in their region right now — see availability note below.

---

## 1. Head-to-head

| Dimension | Raspberry Pi Zero 2 W | ESP32 (generic / all-in-one) |
|---|---|---|
| Price (board only) | $15 official; **street price 2x+ ($30-45) due to 2026 shortage** | $5-12 bare dev board; $18-45 all-in-one w/ display |
| 2026 availability | Poor — no US distributors in stock as of June 2026 per rpilocator; substrate/AI-boom supply crunch; production continues to 2030 but spot stock is scarce | Good — ESP32 family in full mass production, many vendors, no reported shortage |
| Idle power | ~0.6-1.25 W (100-250 mA @5V) headless | ~0.1-0.5 W typical (WiFi-connected, screen on); microcontroller-class draw |
| Boot time | 15-30+ s (full Linux boot) | ~1-12 s cold boot (ESPHome default configs) |
| WiFi reliability | Full Linux network stack — mature, auto-reconnect via NetworkManager/wpa_supplicant | Needs explicit reconnect logic; ESPHome handles this for you; raw Arduino sketches do not by default |
| Always-on suitability | Fine electrically; main risk is SD card wear/corruption on power loss | Designed for always-on embedded use; flash wear is a non-issue at this write rate |
| Heat | Warm to touch under load, negligible at idle for this task | Cool — no heatsink ever needed |

Sources: [Raspberry Pi Zero 2 W product page](https://www.raspberrypi.com/products/raspberry-pi-zero-2-w/), [rpilocator](https://rpilocator.com/), [CNX Software power deep-dive](https://www.cnx-software.com/2021/12/09/raspberry-pi-zero-2-w-power-consumption/), [Jeff Geerling power blog](https://www.jeffgeerling.com/blog/2021/disabling-cores-reduce-pi-zero-2-ws-power-consumption-half/), [ESPHome community boot-time thread](https://community.home-assistant.io/t/esp32-expected-boot-time/693266), [espboards.dev WiFi reconnect issue](https://www.espboards.dev/troubleshooting/issues/wifi/esp32-wifi-reconnect-issue/).

## 2. Display options

| Platform | Attach method | Typical size/res | All-in-one exists? | Soldering? |
|---|---|---|---|---|
| ESP32 | SPI TFT, small OLED (I2C), e-paper (SPI) | 0.9"-2.8" TFT (240x135 to 320x240 common); 1.9" @170x320 on T-Display-S3 | **Yes** — LilyGO T-Display-S3 (~$23, color IPS, USB-C, no solder), M5Stack Core2 (~$43, 2.0" capacitive touch, USB-C, no solder), M5Stack CoreS3 (similar, camera+mic extras you don't need) | None for these boards — headers/screen pre-soldered at factory |
| Pi Zero 2 W | HDMI (mini-HDMI, needs adapter/monitor), SPI TFT HATs, e-paper HATs, official DSI touchscreen | HDMI = any monitor (overkill, needs separate power+monitor); SPI TFTs 2.4"-3.5"; e-paper 2.13"-7.5" | Pimoroni HyperPixel (SPI TFT, no-solder HAT, ~$35-45), Waveshare e-paper HAT+ (~$11-27 depending on touch/case) | No solder needed for HAT-style boards (pin-header stack); GPIO header sometimes needs soldering on the bare Zero 2 W SKU (buy the "with header" variant to avoid this) |

Notes:
- E-paper is a poor fit here: refresh is slow (seconds) and ghosting/full-refresh flicker is visible — bad UX for a value that changes every 1-2 min and that you want to glance at instantly. Fine for battery devices, not for this.
- HDMI on Pi Zero 2 W means a full monitor — defeats "small desk display" framing unless pairing with a small HDMI panel (adds cost/bulk, plus you're now running a compositor/X11/Wayland just to show a number).
- The ESP32 all-in-one boards are the most direct match to "small always-on screen showing a percent + bar."

Sources: [LilyGO T-Display-S3](https://lilygo.cc/products/t-display-s3), [espboards.dev T-Display-S3 specs](https://www.espboards.dev/esp32/lilygo-t-display-s3/), [M5Stack Core2 shop](https://shop.m5stack.com/products/m5stack-core2-esp32-iot-development-kit), [M5Stack CoreS3 shop](https://shop.m5stack.com/products/m5stack-cores3-esp32s3-iotdevelopment-kit), [Waveshare 2.13" e-Paper HAT](https://www.waveshare.com/2.13inch-e-paper-hat.htm), [Waveshare 2.13" Touch e-Paper HAT w/ case](https://www.waveshare.com/2.13inch-touch-e-paper-hat-with-case.htm).

## 3. Software/dev experience — fastest path to "poll JSON, draw a bar"

| Approach | Effort to first working prototype | Notes |
|---|---|---|
| **ESPHome (declarative YAML)** | **Lowest** — `http_request` + `json` components + a `display` lambda; no C++ toolchain, flashes over USB once then OTA forever | Best fit if willing to add Home Assistant OR run ESPHome dashboard standalone (HA not required, ESPHome CLI/dashboard works alone) |
| Arduino/C++ on ESP32 | Low-medium — `HTTPClient` + `ArduinoJson` + a display driver lib (TFT_eSPI / LVGL); more control, more boilerplate | Good if the user wants full custom UI/animations later |
| MicroPython on ESP32 | Low-medium — `urequests` + `ujson` + a display driver; Python-familiar dev will feel at home fast | Slightly less mature display driver ecosystem than Arduino/C++ for these specific boards |
| Pi Zero 2 W, Python + any display lib | Low — full Python, `requests`, Pillow/luma.lcd for SPI TFT, or just a browser in kiosk mode hitting a local HTML page | Fastest *if* the user is purely a software dev with zero embedded experience — literally the same skills as writing a normal script; no flashing, no YAML DSL |
| Pi Zero 2 W, browser kiosk (Chromium) | Medium — heavier RAM/CPU footprint on Zero 2 W (512MB RAM), boot time to a running browser is tens of seconds | Only worth it if richer web-based UI wanted |

Given the user is "comfortable with code, less so with electronics" — ESPHome is arguably *lower* electronics risk than a bare Pi (no wiring HAT stacks correctly, no worrying about GPIO pinout) because the T-Display-S3/M5Stack boards are single units. The YAML-based ESPHome config for a JSON-polled display is a well-trodden path in the Home Assistant community (see `http_request` component docs).

Sources: [ESPHome http_request component](https://esphome.io/components/http_request/), [ESPHome json component](https://esphome.io/components/json/), [ESPHome community "read JSON from web"](https://community.home-assistant.io/t/esphome-how-to-read-json-from-web/304034).

## 4. Reliability for unattended always-on operation

| Risk | Pi Zero 2 W | ESP32 |
|---|---|---|
| Storage corruption | SD cards not designed for constant power-cycling/writes; power loss during write can corrupt filesystem — well-documented forum issue. Mitigation: read-only root + overlayfs or log2ram, but adds setup complexity and quirks (changes don't persist without disabling overlay) | Flash wear negligible at this write rate (no OS, no logs, config baked into firmware); nothing to corrupt from power loss mid-poll |
| Recovery from power loss | Needs full Linux boot + service restart; usually fine but slower, and repeated ungraceful shutdowns are the actual corruption vector | Instant re-boot to running state, no filesystem-consistency concern |
| WiFi loss recovery | Handled by mature Linux network stack automatically | ESPHome has built-in reconnect/backoff logic; raw Arduino code needs you to add it (not hard, ~10 lines) |
| OTA updates | Standard Linux update tooling (apt, or just re-flash SD) | ESPHome has native OTA out of the box; Arduino has ArduinoOTA/ElegantOTA/HTTP-OTA options, all mature |
| Watchdog | Available via systemd/software watchdog config (extra setup) | ESPHome enables hardware watchdog by default |

Net: for a device meant to sit untouched for weeks, ESP32 has fewer moving parts that can fail. Pi Zero's known failure mode (SD corruption) is exactly the "weeks of uptime, occasional power blip" scenario this project describes.

Sources: [core-electronics read-only Pi guide](https://core-electronics.com.au/guides/read-only-raspberry-pi/), [Raspberry Pi forums overlay/corruption thread](https://forums.raspberrypi.com/viewtopic.php?t=294427), [ESPHome OTA docs](https://esphome.io/components/ota/esphome/).

## 5. Integration angle — how the device gets data from the Mac

| Approach | Effort | Fit |
|---|---|---|
| **(a) Mac runs tiny local HTTP/JSON endpoint, device polls it** | Low — VibeProxy (Tauri/Rust) already computes usage %; expose it on a local port (e.g. `127.0.0.1`/LAN-bound `axum`/`warp` route or even a static JSON file served by a one-line Rust or Python HTTP server). Device does a GET every 1-2 min. | **Recommended.** No new infra dependency, works with either ESP32 or Pi, simplest mental model, matches "low data rate" constraint well. |
| (b) MQTT / Home Assistant | Medium if HA not already running (new service, broker, entities to define); trivial if HA already deployed | Only worth it if user *already* runs Home Assistant — then this becomes near-zero-effort since ESPHome devices are first-class HA citizens and you get history/graphing for free. If HA isn't already running, this is a heavier dependency than needed for a 2-value gadget. |
| (c) ESPHome (as the firmware, independent of (a)/(b)) | Low | ESPHome is the *device* framework, orthogonal to transport — pairs with (a) via `http_request` or with (b) via native MQTT/HA API. Recommended pairing: ESPHome + (a) if no HA; ESPHome + HA-native API if HA exists. |

Given no evidence the user runs Home Assistant already (not mentioned, no HA references in project memory), **(a) HTTP/JSON polling is the right default** — skip MQTT/HA entirely unless they confirm HA is already part of their setup.

## Suggested shopping list (recommended path)

| Item | Price (approx) | Why |
|---|---|---|
| LilyGO T-Display-S3 | ~$23 | ESP32-S3, 1.9" color IPS 170x320, USB-C, integrated buttons, no soldering |
| USB-C cable (likely already owned) | $0-5 | Power + initial flash |
| Small stand/case (optional, 3D print or off-shelf acrylic stand LilyGO sells) | $0-10 | Desk presentation |
| **Total** | **~$25-35** | |

Firmware: ESPHome YAML with `http_request` (poll VibeProxy's JSON endpoint every 60-120s) + `json` parse + `display`/`font` + a `lambda` drawing a filled-rect bar. No Home Assistant required to run ESPHome standalone (flash via ESPHome CLI or dashboard, no HA install needed).

## Alternative shopping list (if Pi Zero 2 W preferred)

| Item | Price (approx) | Why |
|---|---|---|
| Raspberry Pi Zero 2 W (with header) | $15 official / likely $30-45 street due to 2026 shortage | Check rpilocator.com for in-stock alerts |
| Pimoroni HyperPixel 2.1" (SPI, no-solder HAT) or Waveshare 2.13" e-paper HAT+ (if refresh-rate tradeoff acceptable) | $35-45 (HyperPixel) / $11-27 (e-paper) | Color TFT (HyperPixel) better UX for frequent updates; e-paper cheaper but slow refresh — avoid for this use case |
| Quality SD card (A1/A2 rated) + read-only overlay setup | $10-15 + setup time | Mitigate corruption risk |
| **Total** | **~$60-100+ setup time** | |

---

## Unresolved questions

1. Does the user already run Home Assistant anywhere in their home network? This flips the integration recommendation for (b)/(c) if yes.
2. Does VibeProxy currently expose (or plan to expose) any local HTTP endpoint at all, or would this be new surface added to the Tauri app? (Not found in current VibeProxy codebase state per prior project memory — appears greenfield as of 2026-07-18.)
3. Physical desk constraints (available USB power source location, desired viewing distance/size) — affects whether 1.9" T-Display-S3 is legible enough vs. a larger M5Stack/HyperPixel screen.
4. Current live regional stock/price for Pi Zero 2 W wasn't checked against the user's specific country/reseller — the shortage note is US-centric; worth a live rpilocator.com check if they lean Pi.
