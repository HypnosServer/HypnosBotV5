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
    block_id, block_meta = get_block("overworld", 19, 6, 397)
    if block_id == 55 and block_meta == 15:
        print("PRINT Overworld switch | ON")
    else:
        print("PRINT Overworld switch | OFF")

def nether_mobswitch():
    block_id, block_meta = get_block("nether", -324, 129, -131)
    if block_id == 55 and block_meta == 15:
        print("PRINT Nether switch | ON")
    else:
        print("PRINT Nether switch | OFF")


def eps_storage():
    _, gp_percent_10 = get_block("nether", 176, 23, 1583)
    _, gp_percent_1  = get_block("nether", 176, 23, 1587)

    _, gp_10  = get_block("nether", 163, 15, 1578)
    if gp_10 > 4:
        gp_10 = 0
    _, gp_1   = get_block("nether", 163, 15, 1582)
    _, gp_dec = get_block("nether", 163, 15, 1588)

    _, bone_percent_10 = get_block("nether", 116, 23, 1585)
    _, bone_percent_1  = get_block("nether", 116, 23, 1581)

    _, bone_10  = get_block("nether", 129, 15, 1590)
    if bone_10 > 4:
        bone_10 = 0
    _, bone_1   = get_block("nether", 129, 15, 1586)
    _, bone_dec = get_block("nether", 129, 15, 1580)

    print(f"PRINT EPS Storage | Gunpowder: {gp_10}{gp_1}.{gp_dec} ({gp_percent_10}{gp_percent_1}%), Bones: {bone_10}{bone_1}.{bone_dec} ({bone_percent_10}{bone_percent_1}%)")





ow_mobswitch()
nether_mobswitch()
eps_storage()
