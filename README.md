# Bluetooth OTS client

[![github](https://img.shields.io/badge/github-katyo/bluetooth--ots--rs-8da0cb.svg?style=for-the-badge&logo=github)](https://github.com/katyo/bluetooth-ots-rs)
[![crate](https://img.shields.io/crates/v/bluez-async-ots.svg?style=for-the-badge&color=fc8d62&logo=rust)](https://crates.io/crates/bluez-async-ots)
[![docs](https://img.shields.io/badge/docs.rs-bluez--async--ots-66c2a5?style=for-the-badge&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K)](https://docs.rs/bluez-async-ots)
[![MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-brightgreen.svg?style=for-the-badge)](https://opensource.org/licenses/apache-2-0)
[![CI](https://img.shields.io/github/actions/workflow/status/katyo/bluetooth-ots-rs/ci.yml?branch=master&style=for-the-badge&logo=github-actions&logoColor=white)](https://github.com/katyo/bluetooth-ots-rs/actions?query=workflow%3ARust)

This crate implements Bluetooth Object Transfer Service (OTS).
Implementation compatible with [OTS 1.0](https://www.bluetooth.com/specifications/specs/object-transfer-service-1-0/) specification.
