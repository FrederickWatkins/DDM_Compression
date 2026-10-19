---
title: MQ00 Protocol
abbrev: MQ00
docname: mq00-compression-00
category: info
ipr: trust200902
submissiontype: independent
workgroup: None
date: 2026-10-18
author:
  - ins: F. Watkins
    name: Frederick Watkins
    organization: University of Cambridge
---

--- abstract

This document specifies a binary protocol for encoding and decoding
Delay-Doppler Map (DDM) data in the MQ00 format.

--- middle

# Introduction

This document defines an implementation-independent binary protocol
for representing compressed DDM frames.

The keywords "MUST", "MUST NOT", "SHOULD", and "MAY" in this
document are to be interpreted as normative requirements.

# Conventions and terminology

## Byte order

Unless explicitly specified otherwise, multibyte integers MUST be
encoded in big-endian byte order.

# Packet format

## Header layout

| Offset | Type   | Description |
| ------ | ------ | ----------- |
| 0      | UINT32 | Packet preamble = 0xCA750200 |
| 4      | UINT32 | Packet type = 0x4D513030 for MQ00 |
| 8      | UINT8  | Header version = 0 for the version described in this document |
| 9      | UINT8  | DDM hardware channel number |
| 10     | UINT8  | Status flags |
| 11     | UINT8  | Wavelet coefficient quantisation/wavelet transform stages |
| 12     | UINT8  | Reserved |
| 13     | UINT8  | Packet type |
| 14     | UINT16 | DC offset |
| 16     | UINT32 | Time stamp of integration start: GPS seconds at last PPS as rounded integer |
| 20     | INT32  | Time stamp of integration start: sample clocks since last PPS |

## Detailed metadata descriptions

- Wavelet coefficient quantisation/wavelet transform stages:
- Packet type

# Pixel data encoding

## Decoding process

## Encoding process (informative)

For the purposes of protocol standardization, a compliant encoder is any encoder that produces a
bitstream that a compliant decoder (complying with the MUST directives above) can reproduce the
desired data from. Below is an example of a compliant encoder.

--- back

# Acknowledgements

None.

# References

None.
