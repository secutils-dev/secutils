# Secrets rotation policy

Applies to every credential a service uses: API keys, database passwords, signing keys,
and OAuth client secrets.

## Rules

1. Store secrets in the vault only. Never in the repository, a ticket, or a chat message.
2. Rotate on a schedule: every 90 days for API keys and passwords, every year for signing keys.
3. Rotate immediately when a person with access leaves the team, or when a secret may have leaked.
4. Use two active versions during a rotation, so services pick up the new one before the old one is revoked.

## How to rotate a database password

1. Create the new password in the vault as the next version.
2. Add it to the database as a second login for the same role.
3. Restart the services one at a time; each reads the newest version at startup.
4. When no connection uses the old login for 24 hours, drop it.

## Audit

The security team reviews the vault access log every month and lists secrets older than
their rotation period.

<!-- su-comments
[
  {
    "id": "0b6f2d84",
    "quote": "every 90 days for API keys and passwords",
    "comments": [
      { "id": "e4a17c39", "author": "Noor", "at": "2026-06-18T16:22:00.000Z", "body": "Can the vault rotate these automatically? Manual rotation every quarter will be forgotten." }
    ]
  }
]
-->
