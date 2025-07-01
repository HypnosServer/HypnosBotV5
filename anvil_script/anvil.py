import sys

def get_block(dim, x, y, z) -> tuple:
    print(f"GET {dim} {x} {y} {z}")
    metadata = input()
    try:
        block_id, block_meta = metadata.split()
    except e:
        sys.exit(1)
    return int(block_id), int(block_meta)

def ow_mobswitch():
    block_id, block_meta = get_block("overworld", 0, 0, 0)
    if block_id == 100:
        print("PRINT Overworld switch | ON")
    else:
        print("PRINT Overworld switch | OFF")

def nether_mobswitch():
    block_id, block_meta = get_block("overworld", 0, 16, 0)
    if block_id == 200:
        print("PRINT Nether switch | ON")
    else:
        print("PRINT Nether switch | OFF")


ow_mobswitch()
nether_mobswitch()
