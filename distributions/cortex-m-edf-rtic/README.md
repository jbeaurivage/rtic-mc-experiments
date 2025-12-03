# EDF scheduler RTIC distribution for Cortex-M devices

## Chip requirements:

- Has DWT with cycle counting

## Implementation details

This distribution uses the DWT cycle counter to compute the absolute task deadlines. Manually resetting the CYCCNT register will mess up the scheduling.
