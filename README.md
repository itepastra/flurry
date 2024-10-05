Flurry is a pixelflut compatible server written in rust 
with a focus on minimizing latency while keeping high performance.

## Protocols

Multiple protocols are supported:
- Text: The default protocol, it is compliant with pixelflut but it defines some extra commands
    - CANVAS <id>: used to change to a completely seperate canvas, the amount and size is defined by the host
    - PROTOCOL <protocol name>: used to change to different protocols, the useable names are:
        - text: goes to the Text protocol
        - binary: goes to the Binary protocol
- Binary: A binary analog to the text version, about twice as efficient with bandwidth, the commands are
    - size: 0x73 <u8 canvas> -> <u16_le x> <u16_le y>
    - help: 0x68 -> help message (in UTF-8)
    - get pixel: 0x20 <u8 canvas> <u16_le x> <u16_le y> -> <u8 red> <u8 green> <u8 blue>
    - set pixel rgb: 0x80 <u8 canvas> <u16_le x> <u16_le y> <u8 red> <u8 green> <u8 blue>
    - blend pixel rgba: 0x81 <u8 canvas> <u16_le x> <u16_le y> <u8 red> <u8 green> <u8 blue> <u8 blend>
    - set pixel grayscale: 0x82 <u8 canvas> <u16_le x> <u16_le y> <u8 white>


