# Privacy Policy

KineAnchor collects no data. Ever.

## What this means

- **No data leaves your devices.** The app never connects to the internet, has no servers, no analytics, no telemetry, no crash reporting, and no advertising.
- **No accounts, no tracking.** There is nothing to sign up for and nothing to identify you.
- **No third parties.** No SDK, library, or service receives anything from the app.

## Network use

The only networking KineAnchor does is between *your own* devices: your phone and your desktop exchanging motion sensor data over USB tethering or your local Wi-Fi. This traffic is peer-to-peer on your local network, secured with a pairing token, and never reaches the internet.

## Permissions

- **Display over other apps** — required to draw the overlay dots. Used only for that.
- **Camera** — optional, only to scan the desktop's pairing QR code. Frames are decoded on your device and immediately discarded. You can instead type the pairing code by hand.
- **Foreground service** — keeps the sensor stream running while the screen is off.

## Data stored on your device

KineAnchor saves only what it needs to work: your settings and the pairing token for your own desktop. This stays on your device and is never transmitted anywhere except, as described above, to your own paired desktop. Deleting the app deletes it.

## Verify it yourself

KineAnchor is free software under GPL-3.0-or-later. You do not have to trust this page — read the code at
[github.com/nandazahran/KineAnchor](https://github.com/nandazahran/KineAnchor) or build it yourself.

## Changes

If a future version ever changed any of this, it would be a major change to the app's core promise, would be announced in the release notes, and this page would be updated before that version shipped.
