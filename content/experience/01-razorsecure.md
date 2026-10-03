---
title: Senior Rust Software Engineer & Platform Lead
org: RazorSecure
location: Remote
date: December 2019 – Present
emoji: "🚀"
accent: yellow
---
- **Platform technical lead:** Set the technical direction for the product and a small engineering team, and own the cross-platform roadmap across the on-train agent, frontend and backend platforms, so an architectural decision on one doesn't break another. Recent designs include push-based configuration management, offline (air-gapped) agent deployment and an RBAC migration.
- **Rust rewrite of the on-train detection agent:** Led the agent's migration from Python to Rust, now complete and running on 1,000+ trains, as an incremental strangler-fig rewrite that shipped one stage per sprint with no big-bang cutover. Built a supervised Tokio runtime in which a faulty monitor restarts rather than taking the agent down, reached parity across the full monitor suite, replaced C-FFI dependencies with pure Rust, and productionised rail-network protocol monitoring end to end.
- **High-performance detection core:** Designed and built a zero-copy Rust deep packet inspection library with eBPF (XDP) acceleration, sustaining sub-microsecond latency at over 1 million packets per second, plus an L2–L7 firewall for an embedded rail security gateway.
- **Cloud portability, on-prem and release engineering:** Made the whole platform deployable on Azure, on-prem and in air-gapped Kubernetes clusters alongside the cloud offering. Consolidated Helm deployments for around 18 microservices into a single platform chart, moved CI onto GitHub Actions, built release automation, and stood up a GitOps-managed on-prem test rack (Talos Linux, Flux).
- **Data-store performance and observability:** Led PostgreSQL alert-store tuning (functional indexes, partition pruning, bulk inserts) that eliminated out-of-memory failures in large batch jobs, alongside MySQL, Elasticsearch and RabbitMQ upgrades. Built an authenticated Grafana proxy and instrumented the platform with Datadog APM tracing.
- **Test-evidence platform:** Designed and built, from scratch, the platform the company now uses for all of its manual test execution and evidence capture: a Rust (Loco) API and React/TypeScript app with a versioned test library, immutable test-plan snapshots, requirements traceability, role-based sign-off with SSO, an MCP server for AI agents and PDF test reports.
- **Engineering and test tooling:** Built a native Rust (GPUI) desktop configurator for agent and monitor configuration, ratatui traffic generators that produce realistic train-network and GPS data along real rail routes, and a multi-tier BDD harness that runs end to end through a real MQTT broker in CI.
