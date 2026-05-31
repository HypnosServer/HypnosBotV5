import sys
import requests

def get_block(dim, x, y, z) -> tuple:
    print(f"GET {dim} {x} {y} {z}")
    metadata = input()
    try:
        block_id, block_meta = metadata.split()
    except e:
        sys.exit(1)
    return int(block_id), int(block_meta)

def get_perim(dim, x, y, z) -> tuple:
    print(f"PERIM {dim} {x} {y} {z}")
    stats = input()
    try:
        block_cnt, air_cnt = stats.split()
    except e:
        sys.exit(1)
    return int(block_cnt), int(air_cnt)


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
        gp_10 = ""
    _, gp_1   = get_block("nether", 163, 15, 1582)
    _, gp_dec = get_block("nether", 163, 15, 1588)

    _, bone_percent_10 = get_block("nether", 116, 23, 1585)
    _, bone_percent_1  = get_block("nether", 116, 23, 1581)

    _, bone_10  = get_block("nether", 129, 15, 1590)
    if bone_10 > 4:
        bone_10 = ""
    _, bone_1   = get_block("nether", 129, 15, 1586)
    _, bone_dec = get_block("nether", 129, 15, 1580)

    print(f"PRINT EPS Storage | Gunpowder: {gp_10}{gp_1}.{gp_dec}M ({gp_percent_10}{gp_percent_1}%), Bones: {bone_10}{bone_1}.{bone_dec}M ({bone_percent_10}{bone_percent_1}%)")

def leaderboard():
    r = requests.get("http://localhost:11002/api/weekly?objectives=dugged,u.diamond_shovel")
    js = r.json()
    string = "PRINT Weekly Dig Leaderboard | "
    for stat in js:
        player = stat['player']
        gain = stat ['gain']
        string += f"{player}: {gain}\\n"
    print(string)

def peri():
    block_cnt, air_cnt = get_perim("overworld", 1905, 94, -2865)
    percent = block_cnt / air_cnt
    left = air_cnt - block_cnt
    print(f"Jepstein | ~{percent:.2f} done \\n {left} non-air left")



ow_mobswitch()
nether_mobswitch()
eps_storage()
peri()
leaderboard()
