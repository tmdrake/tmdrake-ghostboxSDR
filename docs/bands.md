# Analog voice on an RTL-SDR

Generic R820T sticks start around **24 MHz**. That is the floor without an upconverter.

| Preset | MHz | Demod | Why |
|---|---|---|---|
| FM Broadcast | 88–108 | WFM | Classic shack-hack |
| VHF Low | 30–50 | NFM | Lowest analog voice without HF |
| 6 m ham | 50–54 | NFM | FM simplex |
| Airband | 118–137 | AM | Towers / approach |
| 2 m ham | 144–148 | NFM | Repeaters, 146.520 |
| VHF High | 150–174 | NFM | Analog fire/EMS/sheriff |
| Marine VHF | 156–162 | NFM | Ch 16 = 156.800 |
| NOAA Weather | 162.400–162.550 | NFM | Seven analog channels |
| 70 cm ham | 420–450 | NFM | 446.000 simplex |
| GMRS / FRS | 462–467 | NFM | Handhelds |
| AM Broadcast | 0.53–1.70 | AM | **Needs HF** (V4 or direct sampling) |

Skip in v1: 700/800 MHz public safety (mostly P25, often encrypted).

County fire/sheriff frequencies live on RadioReference. ShackHack ships national rasters, not a county database.
