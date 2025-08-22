# Behemoth Simulator

```
Asset simulator which produces telemetry for a configurable amount of channels at a configurable frequency.

Usage: behemoth_simulator [OPTIONS] --uri <URI> <--apikey <APIKEY>>

Options:
  -a, --asset <ASSET>
          Asset name [default: behemoth_simulator]
  -n, --num-components <NUM_COMPONENTS>
          The number of components the asset has [default: 100]
  -c, --channels-per-component <CHANNELS_PER_COMPONENT>
          The number of channels the asset has [default: 10]
  -f, --frequency <FREQUENCY>
          The desired frequency in which to send data in Hz [default: 1000]
  -k, --apikey <APIKEY>
          Sift API key
  -u, --uri <URI>
          Sift gRPC URL (http/https must be included)
  -d, --disable-tls
          Disables TLS for environments that don't use it
  -h, --help
          Print help
  -V, --version
          Print version
```
