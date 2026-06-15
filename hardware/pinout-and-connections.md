# Cell Symphony Hardware Pinout and Connections

Target platform: Raspberry Pi Zero 2W

## Power

- ADA4090 pin 1 (VBUS) -> +5V rail
- ADA4090 pin 2 (GND) -> GND rail
- Pi Pin 2 (5V) -> +5V rail
- Pi Pin 6 (GND) -> GND rail
- Pi Pin 1 (3.3V) -> OLED VIN
- ADA4090 pins 3/4 (CC1/CC2) unconnected

## I2C Bus (NeoTrellis chain + NeoKey)

- Pi Pin 3 (GPIO2/SDA) -> NeoTrellis SDA, NeoKey SDA
- Pi Pin 5 (GPIO3/SCL) -> NeoTrellis SCL, NeoKey SCL
- NeoTrellis VIN -> +5V
- NeoTrellis GND -> GND
- NeoKey VIN -> +5V
- NeoKey GND -> GND

## OLED (SPI, 16-bit color, SSD1351 / Adafruit 1431)

- Pi Pin 6 (GND) -> OLED GND
- Pi Pin 19 (GPIO10/MOSI) -> OLED MOSI
- Pi Pin 23 (GPIO11/SCLK) -> OLED CLK
- Pi Pin 24 (GPIO8/CE0) -> OLED CS
- Pi Pin 18 (GPIO24) -> OLED DC
- Pi Pin 22 (GPIO25) -> OLED RST

## DAC (I2S, ADA6250 / PCM5102)

- Pi Pin 12 (GPIO18) -> BCK
- Pi Pin 35 (GPIO19) -> LRCK
- Pi Pin 40 (GPIO21) -> DIN
- +5V rail -> DAC VIN
- GND rail -> DAC GND
- FMT/XSMT unconnected

## Rotary Encoders (Bourns PEC12R, clickable)

### SW1 (Encoder Main)

- A -> Pin 29 / GPIO5
- B -> Pin 31 / GPIO6
- SW -> Pin 32 / GPIO12
- C -> GND

### SW2 (Encoder Aux 1)

- A -> Pin 33 / GPIO13
- B -> Pin 36 / GPIO16
- SW -> Pin 11 / GPIO17
- C -> GND

### SW3 (Encoder Aux 2)

- A -> Pin 13 / GPIO27
- B -> Pin 7 / GPIO4
- SW -> Pin 38 / GPIO20
- C -> GND

### SW4 (Encoder Aux 3)

- A -> Pin 37 / GPIO26
- B -> Pin 16 / GPIO23
- SW -> Pin 15 / GPIO22
- C -> GND

### SW5 (Encoder Aux 4)

- A -> Pin 8 / GPIO14
- B -> Pin 21 / GPIO9
- SW -> Pin 26 / GPIO7
- C -> GND

## Firmware/Runtime Notes

- I2C devices:
  - NeoTrellis 4x4 x4 (daisy-chained, address-jumpered)
  - NeoKey 1x4
  - all on I2C bus 1
- NeoTrellis is driven via seesaw over I2C (no direct NeoPixel data line).
- NeoKey also uses seesaw over I2C.
- OLED driver is SSD1351 (16-bit color).
- DAC (ADA6250 connector, PCM5102 module) is over I2S (BCK/LRCK/DIN only).
- Encoders are direct GPIO with internal pull-ups + software debounce.
- GPIO9 (SPI MISO) is reused for SW5 channel B because OLED is write-only.

## Logical Input Mapping

- SW1 -> `encoder_main`
- SW2 -> `encoder_aux_1`
- SW3 -> `encoder_aux_2`
- SW4 -> `encoder_aux_3`
- SW5 -> `encoder_aux_4`

- NeoKey key1 -> `button_a` / Back
- NeoKey key2 -> `button_s` / Space
- NeoKey key3 -> `button_shift`
- NeoKey key4 -> `button_fn`
