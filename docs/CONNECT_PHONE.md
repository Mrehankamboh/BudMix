# Connecting your phone

BudsLink can take your phone's audio two ways. Nothing is installed on the
phone in either case.

## Over Wi-Fi: Cast (Miracast)

The phone sends its screen and sound with Android's built-in cast feature.
BudsLink shows up in the phone's list as **BudsLink**.

Miracast uses Wi-Fi Direct, a direct link between the phone and the laptop, so
the router is not involved. Keep Wi-Fi turned **on** on both devices.

### Infinix Hot 11 Play (XOS, Android 11)

The menu names below are the usual ones on Infinix/Tecno phones; they have
not been checked on this exact model yet (item M0-3 of the
[hardware checklist](HARDWARE_TESTING.md) does that).

1. Turn on Wi-Fi.
2. Swipe down twice from the top to open Quick Settings and look for a tile
   named **Cast** or **Screen cast**. If it is not there, tap the pencil
   (edit) icon and drag the tile in.
   Or: **Settings**, search for **cast**, open **Cast**.
3. If the list is empty: tap **⋮** (three dots, top right) and tick
   **Enable wireless display**. This option is hidden until you open that
   menu.
4. Wait a few seconds and tap **BudsLink**.
5. If the laptop shows a PIN, type it on the phone.
6. Play something on the phone. The phone's speaker should go quiet and the
   sound should come from the buds.

To stop: open Cast again and tap **BudsLink → Disconnect**, or use the cast
notification.

### Other Android phones

- **Samsung:** Quick Settings → **Smart View** → BudsLink.
- **Xiaomi / Redmi / POCO:** Settings → Connection & sharing → **Cast**.
- **OPPO / realme / OnePlus:** Settings → Connected devices / Bluetooth & device
  connection → **Screencast** or **Wireless display**.
- **Google Pixel:** recent Pixels removed Miracast. Use the Bluetooth way below.

## Over Bluetooth (automatic fallback)

Pair the phone with the laptop once in **Windows Settings → Bluetooth &
devices → Add device**. From M4 on, BudsLink connects to the paired phone by
itself whenever the phone is not casting, so you don't need to touch the
phone.
