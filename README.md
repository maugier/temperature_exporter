# temperature_exporter

A Prometheus exporter for EnOcean temperature probes.

## Limitations

Only profile A5-02-05 (Temperature Sensor Range 0°C to +40°C) is supported. Other 4BS sensors will be uncorrectly calibrated.

## Usage

The config file requires three things: a listen address for the HTTP endpoint, a serial port connected to an EnOcean device,
and a table of devives.

```
listen: 127.0.0.1:8898
port: COM5
devices:
  "0180ABCD": "main room"
  "0180EFFF": "entrance hall"
```

The exporter exposes a single metric, `enocean_temperature_celsius`, with an `address` label for the address of the device, and optionally, 
a `name` label if the address was listed in the device table.