# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Everyday listeners who want YouTube Music as a proper desktop app on Windows (and macOS/Linux). They aren't power users. They expect the polish of a first-party music app: things just work, lyrics are there, ads aren't.

## Product Purpose

Oxide is a YouTube Music desktop client written in Rust (Tauri). It wraps music.youtube.com and adds what the website lacks: synced lyrics (side panel and an on-screen overlay), ad blocking, media keys and taskbar controls, Discord status, a downloader, and an offline library that plays downloaded songs with their lyrics when there's no internet. Success is a listener never needing to open the website in a browser again.

## Positioning

A native desktop shell around the real YouTube Music, not a reimplementation: the listener keeps their account, library and recommendations, and gets lyrics that work everywhere, including on top of games and offline.

## Operating Context

- Runs in a frameless window with a 32 px app title bar (menus, window buttons) drawn by Oxide.
- Often in the background: controlled from the tray, taskbar thumbnail buttons, media keys and global shortcuts while the listener does something else (games, work).
- The offline player appears when YouTube Music can't be reached, or from App → Offline library. It must work with no network at all.

## Capabilities and Constraints

- Offline player: lists downloaded songs, plays them, shows synced lyrics saved at download time, reports playback to the app so media keys, tray, taskbar buttons and the lyrics overlay keep working.
- Fully offline: no web fonts, CDNs or remote images on the offline page; everything bundled.
- Keyboard and media keys must work everywhere (space, arrows, Ctrl+L for lyrics, media keys, tray, taskbar).
- Must stay fast with hundreds to thousands of downloaded songs.
- Cover art is not stored with downloads yet (thumbnails are embedded in the MP3s only).

## Brand Commitments

- Name: Oxide. The offline player should clearly belong to Oxide, not imitate YouTube Music's look.

## Evidence on Hand

- No testimonials, user counts or press. Don't invent any.

## Product Principles

1. Lyrics are a headline feature: prominent, readable, in sync.
2. Calm and focused over busy: a player first, a library second.
3. Everything reachable from the keyboard and media keys.
4. Nothing breaks offline.
