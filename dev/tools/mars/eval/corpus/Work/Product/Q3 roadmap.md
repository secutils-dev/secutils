# Q3 roadmap

Three themes this quarter: enterprise readiness, mobile, and faster reports.

```mermaid
gantt
    title Q3
    dateFormat YYYY-MM-DD
    section Enterprise
    SAML login           :a1, 2026-07-01, 30d
    Audit log export     :a2, after a1, 20d
    section Mobile
    Offline mode         :b1, 2026-07-06, 45d
    Push notifications   :b2, after b1, 15d
    section Reports
    Query engine rewrite :c1, 2026-07-01, 60d
```

## Enterprise readiness

Single sign-on and an exportable audit log are the two most common blockers in large deals.

## Mobile

Field teams lose signal in warehouses. Offline mode keeps the last 30 days of orders on the
phone and syncs changes when the connection returns.

## Faster reports

Monthly reports take up to two minutes for large accounts. The goal is under ten seconds at
the 95th percentile.

## Not this quarter

Dark mode, a public API v2, and the marketplace.
