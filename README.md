# Envryn Vault

JUST THE UI NOT THE IMPLEMENTATION. THIS IS FOR A DESKTOP APP 

Design the complete Windows desktop UI for Envryn.

Envryn is a local-first developer secrets vault for securely storing:

API keys

environment variables

access tokens

database credentials

SSH credentials

OAuth/client secrets

webhook secrets

developer secure notes

other custom secrets

For now, design ONLY the PC/Windows application.

Do not design mobile screens yet.

Overall Design Direction

Envryn should feel:

simple

clean

serious

trustworthy

private

developer-focused

compact

polished

native-app-like

The interface should feel like a professional desktop utility that a developer would trust with sensitive credentials.

Think about the cleanliness and discipline of apps such as:

Linear

Raycast

1Password

GitHub Desktop

modern IDE settings

Use them only as quality references. Do not copy their layouts or branding.

Very Important: Avoid AI Slop

Do NOT create a typical AI-generated SaaS dashboard.

Avoid:

giant headings

excessive gradients

glowing purple backgrounds

glassmorphism everywhere

huge cards

cards inside cards

excessive rounded containers

random decorative blobs

marketing-style hero sections

fake statistics

charts

security scores

unnecessary badges

excessive explanatory text

huge empty spaces

excessive shadows

excessive animations

emojis as interface icons

unnecessary illustrations

This is a desktop tool, not a website.

Every element should have a purpose.

Visual Style

Use a restrained modern dark interface.

Suggested direction:

Background

Near-black / deep charcoal.

Not pure #000000.

Surfaces

Slightly lighter than the background.

Use subtle differences rather than heavy card borders.

Borders

Thin and low contrast.

Accent

Use one primary accent color.

Do not cover the interface with it.

Use accent primarily for:

primary buttons

selected navigation

keyboard focus

active environment

important actions

Status colors

Use restrained:

green → success

amber → warning

red → danger

Never rely on color alone.

Typography

Use a clean sans-serif such as:

Geist

or a similarly restrained modern typeface.

Use a monospace font such as:

IBM Plex Mono

for:

environment variable names

secret values

fingerprints

SSH information

code-related fields

Typography should be compact.

Avoid oversized text.

Main Desktop Structure

Use a traditional desktop application layout:

┌───────────────────────────────────────────────────────────┐

│ Envryn                                                    │

├──────────────┬────────────────────────────────────────────┤

│              │                                            │

│ Sidebar      │ Main Content                               │

│              │                                            │

│              │                                            │

│              │                                            │

└──────────────┴────────────────────────────────────────────┘

Recommended target design size:

1440 × 900

Also make the layout work reasonably around:

1280 × 720

Sidebar

Keep the sidebar narrow and simple.

Approximately:

220–240px

Example:

Envryn

VAULT

All Secrets

Projects

CATEGORIES

API & Tokens

Databases

SSH

Secure Notes

DEVICES

Trusted Devices

Sync

OTHER

Backup

Settings

──────────────

Vault Locked / Unlocked

Lock Vault

Do not place every item inside separate pills.

Selected navigation can use:

subtle surface background

accent indicator

stronger text

Keep inactive navigation visually quiet.

1. Unlock Screen

When Envryn launches while locked:

                    Envryn

                Vault is locked

             [ Master Password ]

                Unlock Vault

             Use Windows Hello

Keep this screen extremely minimal.

Do not add:

security marketing

illustrations

giant locks

multiple cards

paragraphs explaining encryption

Optional small text:

Your vault stays encrypted until you unlock it.

Nothing more.

2. Main Vault

After unlocking, open to:

All Secrets

Top area:

All Secrets                         + Add Secret

[ Search secrets... ]

All     API Keys     Tokens     Databases     SSH

Below this, show a clean list.

Avoid large cards.

Example:

NAME                         PROJECT        ENVIRONMENT      TYPE

GROQ_API_KEY                 Rescripto      Development      API Key

SUPABASE_SERVICE_ROLE_KEY    Rescripto      Production       API Key

DATABASE_URL                 NameVetta      Production       Database

GitHub Personal Token        Personal       —                Token

VPS Production               Infrastructure Production       SSH

Secret values should NOT appear directly in this overview.

Each row can have actions on hover:

Copy     Reveal     •••

Use icons carefully.

3. Projects

Projects are one of the most important parts of Envryn.

Example screen:

Projects                                      + New Project

Rescripto

Development · 8 secrets

Production · 5 secrets

NameVetta

Development · 6 secrets

Production · 4 secrets

MyGameList

Development · 9 secrets

Production · 3 secrets

Again:

Do not make giant cards.

Use compact project rows or small restrained panels.

4. Project Details

Opening:

Rescripto

Header:

← Projects

Rescripto

Development    Staging    Production

                                   + Add Secret

Environment selection should be prominent but simple.

Below:

Development

NAME                       TYPE                UPDATED

GROQ_API_KEY               API Key             2 days ago

SUPABASE_URL               Environment         5 days ago

DATABASE_URL               Database            Yesterday

JWT_SECRET                 Secret              3 weeks ago

Allow:

search

filter

sort

But keep controls minimal.

5. Secret Detail Panel

Instead of navigating away from the entire project, consider opening secret details in a right-side panel.

Example:

┌────────────────────────────────────────┐

│ GROQ_API_KEY                        ×  │

│                                        │

│ API Key                                │

│                                        │

│ Project                                │

│ Rescripto                              │

│                                        │

│ Environment                            │

│ Development                            │

│                                        │

│ VALUE                                  │

│ ┌────────────────────────────────────┐ │

│ │ ••••••••••••••••••••••••••••     │ │

│ └────────────────────────────────────┘ │

│                                        │

│ [ Copy ]      [ Reveal ]               │

│                                        │

│ Notes                                  │

│ Groq development key                   │

│                                        │

│ Created                                │

│ August 20, 2026                        │

│                                        │

│ Last updated                           │

│ August 20, 2026                        │

│                                        │

│ Edit                                   │

│                                        │

│ Delete Secret                          │

└────────────────────────────────────────┘

Delete should be separated visually from ordinary actions.

6. Reveal Secret

Reveal should require deliberate interaction.

Default:

••••••••••••••••••••••••

Click:

Reveal

Optional secure flow:

Confirm your identity

[Use Windows Hello]

Then temporarily show:

gsk_xxxxxxxxxxxxxxxxx

Automatically hide again after a short period.

Provide:

Hide

Do not leave secrets visible indefinitely.

7. Copy Secret

After copy:

Show a small non-invasive toast:

Secret copied

Clipboard clears in 30 seconds.

Do NOT show the actual secret inside the notification.

8. Add Secret

Use a focused modal or side panel.

Example:

Add Secret

Name

[GROQ_API_KEY]

Type

[API Key ▾]

Project

[Rescripto ▾]

Environment

[Development ▾]

Secret Value

[••••••••••••••••••••••]

Notes

[Optional]

Tags

[Optional]

Cancel                         Save Secret

Keep it simple.

Do not put every possible advanced field on the first screen.

Different secret types can reveal relevant optional fields.

9. Secret Types

Support templates for:

API Key

Name

API Key

Provider

Environment Variable

Variable Name

Value

Access Token

Name

Token

Expiration (optional)

Database

Name

Host

Port

Database

Username

Password

Connection String

SSH

Name

Host

Username

Private Key

Passphrase

Fingerprint

OAuth

Client ID

Client Secret

Secure Note

Title

Content

Custom

Allow custom fields.

Do not overwhelm the UI.

10. Search

Global search should be accessible near the top of the interface.

Search:

secret names

project names

environments

tags

categories

provider names

Never search/display plaintext secret values.

Example results:

GROQ_API_KEY

Rescripto · Development

API Key

DATABASE_URL

NameVetta · Production

Database

Support keyboard shortcut:

Ctrl + K

if appropriate.

11. Categories

Examples:

API & Tokens

OpenAI API

GitHub Token

Groq API

Supabase Key

Databases

NameVetta Production

Rescripto Development

SSH

Production VPS

GitHub SSH

Home Server

Secure Notes

Simple list.

Do not duplicate unnecessary navigation if categories are not useful.

12. Trusted Devices

Design:

Trusted Devices

Only devices you approve can sync with this vault.

Android Phone

Trusted

Last synced 2 min ago

Fingerprint

3F:82:91:A4:...

                           View Details

Another:

Development Laptop

Trusted

Last synced Yesterday

Primary action:

Pair Device

13. Pair Device

Desktop screen/modal:

Pair a Device

Scan this QR code using Envryn

on your other device.

      [ QR CODE ]

Verification Code

481 927

Waiting for device...

Once detected:

Android Phone wants to connect.

Verification code

481 927

Make sure this code matches

the other device.

Cancel                        Trust Device

Keep this interaction very clear.

No complicated cryptographic language.

14. Sync

Simple screen:

Sync

Your devices communicate directly over your local network.

Android Phone

Connected

Last sync: 2 minutes ago

                     Sync Now

Success state:

✓ Everything is up to date

Offline:

Android Phone

Offline

Last sync: Yesterday

Failure:

Sync couldn't complete.

Try again when both devices

are on the same local network.

Retry

Do not add:

bandwidth graphs

fake network analytics

technical topology diagrams

15. Device Details

Example:

Android Phone

Status

Trusted

Last Sync

Today, 4:31 PM

Added

August 20, 2026

Fingerprint

3F:82:91:A4:27:...

Device ID

ENV-A39F2C...

───────────────

Revoke Device

Revoking should require confirmation.

16. Revoke Confirmation

Revoke Android Phone?

This device will no longer be allowed

to sync with this vault.

You will need to pair it again

to reconnect.

Cancel                         Revoke Device

Use red only for the destructive action.

17. Backup

Screen:

Backup

Create an encrypted offline copy

of your Envryn vault.

Last Backup

August 18, 2026

envryn-backup-2026-08-18

Create Encrypted Backup

Restore Backup

Do not include:

cloud backup

Google Drive

Dropbox

OneDrive

in this version.

18. Create Backup

Dialog:

Create Encrypted Backup

Protect the backup with a password.

Backup Password

[••••••••••••]

Confirm Password

[••••••••••••]

Cancel                   Create Backup

19. Restore Backup

Simple file selection flow.

After choosing:

Restore Envryn Backup

Backup:

envryn-backup-2026-08-18

Backup Password

[••••••••••••]

Cancel                         Restore

Provide clear error states.

20. Settings

Avoid one giant page.

Use grouped sections.

Security

Auto Lock

5 minutes

Require authentication to reveal secrets

On

Clipboard Clearing

30 seconds

Lock when Windows locks

On

Appearance

Theme

Dark

Potential future:

System / Light / Dark

Sync

Local Device Discovery

On

Data

Backup

Manage Vault

About

Envryn

Version 0.1.0

Open-source licenses

Security documentation

21. Empty Vault

Example:

No secrets yet

Store your first API key, token,

database credential, or other

development secret.

Add Secret

Do not use a giant illustration.

22. Empty Project

No secrets in Development

Add a secret to this environment.

Add Secret

23. No Search Results

No results for "openrouter"

Try another name, project, or tag.

Nothing more.

24. Security Error States

Design clear states for:

Incorrect password

Incorrect password.

Try again.

Secret couldn't decrypt

This secret couldn't be opened.

The stored data may be damaged.

Never display decrypted/debug information.

Unknown Device

Connection rejected.

This device isn't trusted.

Pairing Expired

Pairing code expired.

Generate a new code to continue.

25. Toasts

Design subtle desktop notifications for:

Secret saved

Secret updated

Secret copied

Secret deleted

Vault locked

Sync complete

Backup created

Device paired

Device revoked

Toasts should:

stay small

appear briefly

avoid unnecessary icons

contain no secret values

26. Confirmation Dialogs

Use confirmation dialogs only when needed:

Delete secret

Delete project

Revoke device

Restore backup

destructive vault operations

Do not ask for confirmation for normal everyday actions.

27. Keyboard Support

Because this is a desktop developer tool, design for keyboard use.

Potential shortcuts:

Ctrl + K       Search

Ctrl + N       Add Secret

Ctrl + L       Lock Vault

Ctrl + C       Copy selected secret

               only when deliberate/safe

Esc            Close panel/modal

Show shortcuts subtly where useful.

Do not clutter the UI with them.

28. Interaction States

Design reusable states for:

Buttons

default

hover

pressed

focus

disabled

loading

Inputs

default

hover

focus

filled

error

disabled

Secret Rows

default

hover

selected

Navigation

default

hover

selected

Device

online

offline

syncing

trusted

revoked/error

29. Reusable Components

Build a proper reusable desktop component system for:

sidebar

sidebar item

button

icon button

input

secure input

search field

select

tabs

environment tabs

project row

secret row

secret type indicator

status indicator

modal

right detail panel

toast

confirmation dialog

settings row

switch

device row

empty state

tooltip

Keep the component library small and consistent.

30. Security UX Principles

The UI should communicate security through behavior rather than marketing.

Good:

Vault locked.

Secret copied. Clipboard clears in 30 seconds.

This device isn't trusted.

Bad:

Your secrets are protected with revolutionary military-grade AI-powered zero-trust quantum encryption.

Never use language like that.

31. Density

This is important.

Envryn is a desktop developer tool.

It should have medium/compact information density.

Do not give every row:

32px vertical padding

giant icon

three lines of description

huge card

Prefer something that can comfortably show 8–12 secrets on a normal desktop screen.

32. Desktop Window States

Design the main UI for:

Standard

1440×900

Smaller desktop

1280×720

Consider what happens when the window becomes narrower:

sidebar may shrink

details panel may overlay

table columns may simplify

Do not let the UI completely break below the ideal size.

33. Required Desktop Screens

Create polished designs for:

Unlock Vault

All Secrets

Projects

Project Details

Secret Details

Add Secret

Edit Secret

Reveal Secret state

Search

API & Tokens category

Database category

SSH category

Secure Notes

Trusted Devices

Pair Device

Pairing confirmation

Sync

Device Details

Revoke Device confirmation

Backup

Create Backup

Restore Backup

Settings

Empty Vault

Empty Project

No Search Results

Sync Offline

Sync Failed

Pairing Expired

Error / damaged secret state

Also create the important modal, toast, loading, hover, focus, and selected states.

Final Goal

The finished Envryn desktop interface should feel like:

A focused developer utility built to protect sensitive information.

It should be attractive because it is:

well spaced

consistent

readable

responsive

restrained

carefully designed

Not because it has lots of decoration.

The user should open Envryn and understand how to use it within seconds.

Prioritize:

security → clarity → usability → speed → consistency → visual polish

Keep the entire Windows experience simple, clean and non-AI-looking.

This project was built with [Lovable](https://lovable.dev).

**Live app**: https://local-vault-for-devs.lovable.app

## Build with Lovable

Continue developing this project in the [Lovable editor](https://lovable.dev/projects/f835d6e9-8ee9-4876-bcd5-6c970be45612).

- **Ship faster**: describe what you want to build and Lovable handles the code.
- **Stay in sync**: every change made in Lovable is committed straight to this repository.
- **Full ownership**: this code is yours. Push to `main` on GitHub and your changes sync back into Lovable, ready for your next prompt.

## Development

Prefer working locally? You need Node.js and npm — [install with nvm](https://github.com/nvm-sh/nvm#installing-and-updating).

```sh
git clone <this-repository-url>
cd <repository-name>
npm i
npm run dev
```
