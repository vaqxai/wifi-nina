`wifi-nina`
========

This is a driver for the WiFi Nina-type SPI co-processors commonly used in the embedded world to give WiFi-capabilities to a less capable chip.

Since the original project [dflemstr/wifi-nina](https://github.com/dflemstr/wifi-nina) seems to have become unmaintained, I've picked up the project to bring it back up to speed with the state of Rust in embedded.
Thanks to those who contributed to the original repository. -CGM616

Since the fork [cgm616/wifi-nina](https://github.com/cgm616/wifi-nina) also seems to have become unmainatained, I've picked up the project to enable the features that the UBLOX chip has that aren't available in this library (yet).
First on the agenda is UDP and AP connectivity, since i need them desperately. Later, who knows, maybe we'll have bluetooth?

## License

Uhh, vaqxai's usin' the same license as cgm616, y'all
All commits after [`cc18a6e`](https://github.com/cgm616/wifi-nina/commit/cc18a6e47a822ef016bf2db084ea60f5586c3e64) are licensed under the [MIT license](https://github.com/cgm616/wifi-nina/blob/master/LICENSE) found in this repository.
All commits before and including `cc18a6e` are licensed under the MIT license or the Apache-2.0 license as found in the [original project's `Cargo.toml`](https://github.com/dflemstr/wifi-nina/blob/master/Cargo.toml).
