set rgba
set rgb
set w
get
lock bytes
help
size


RESERVED    01001000
RESERVED    01010011
RESERVED    01010000
help        01101000
size        01110011 {canvas}
lock        00000000 {amt lsb} {amt msb} {lock command} {lock bytes msb} {lock bytes lsb} {lock values}.. {insert values}...
get         00100000 {canvas} {x lsb} {x msb} {y lsb} {y msb}
set rgb     10000000 {canvas} {x lsb} {x msb} {y lsb} {y msb} {r byte} {g byte} {b byte}
set rgba    10000001 {canvas} {x lsb} {x msb} {y lsb} {y msb} {r byte} {g byte} {b byte} {a byte}
set w       10000010 {canvas} {x lsb} {x msb} {y lsb} {y msb} {w byte}