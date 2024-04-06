# TODO: Clean up this whole thing (it's a mess right now). Split stuff up into modules in some reasonable way.
import numpy as np
import random
import graph_tool
import graph_tool.inference
import graph_tool.topology
from logic.rooms.all_rooms import rooms
import json
import logging
from maze_builder.types import reconstruct_room_data, Direction, DoorSubtype
import pickle
import argparse

# PYTHONPATH=. python rando/gen_maps.py session-2022-06-03T17:19:29.727911.pkl-bk30 2100000 2350000

parser = argparse.ArgumentParser(
    'gen_maps',
    'Generate random maps with area assignment')
parser.add_argument('session_file')
parser.add_argument('start_index')
parser.add_argument('end_index')
parser.add_argument('pool')
args = parser.parse_args()

logging.basicConfig(format='%(asctime)s %(message)s',
                    level=logging.INFO,
                    handlers=[logging.FileHandler("train.log"),
                              logging.StreamHandler()])
import torch
import io
import os

class CPU_Unpickler(pickle.Unpickler):
    def find_class(self, module, name):
        if module == 'torch.storage' and name == '_load_from_bytes':
            return lambda b: torch.load(io.BytesIO(b), map_location='cpu')
        else:
            return super().find_class(module, name)

device = torch.device('cpu')

session_name = args.session_file
start_index = int(args.start_index)
end_index = int(args.end_index)
session = CPU_Unpickler(open('models/{}'.format(session_name), 'rb')).load()

max_mc_dist = torch.amax(session.replay_buffer.episode_data.mc_distances, dim=1)
mean_mc_dist = torch.mean(session.replay_buffer.episode_data.mc_distances.to(torch.float), dim=1)

common_mask = (
    (session.replay_buffer.episode_data.reward == 0) &
    session.replay_buffer.episode_data.toilet_good &
    (torch.mean(session.replay_buffer.episode_data.save_distances.to(torch.float), dim=1) < 4.10) &
    (session.replay_buffer.episode_data.graph_diameter <= 45)
)

tame_ind = torch.nonzero(
    common_mask &
    (session.replay_buffer.episode_data.mc_dist_coef > 0.0) &
    (max_mc_dist <= 13)
)
wild_ind = torch.nonzero(
    common_mask &
    (session.replay_buffer.episode_data.mc_dist_coef == 0.0) &
    (max_mc_dist >= 22)
)

def print_summary(ind):
    print("cnt:", len(ind))
    print("save:", torch.mean(session.replay_buffer.episode_data.save_distances[ind].to(torch.float)))
    print("diam:", torch.mean(session.replay_buffer.episode_data.graph_diameter[ind].to(torch.float)))
    print("mean_mc", torch.mean(session.replay_buffer.episode_data.mc_distances[ind].to(torch.float)))
    print("max_mc", torch.mean(torch.amax(session.replay_buffer.episode_data.mc_distances[ind], dim=-1).to(torch.float)))
    print()

print("--- Tame ---")
print_summary(tame_ind)
print("--- Wild ---")
print_summary(wild_ind)

pool = args.pool
if pool == "tame":
    ind = tame_ind
elif pool == "wild":
    ind = wild_ind
else:
    print("Unrecognized pool " + pool)
# ind = wild_ind
logging.info("{} maps".format(ind.shape[0]))
# os._exit(0)

os.makedirs(f'maps/{session_name}-{pool}', exist_ok=True)
episode_data_action = session.replay_buffer.episode_data.action[ind[start_index:end_index], :]
del session

toilet_idx = [i for i, room in enumerate(rooms) if room.name == "Toilet"][0]

def get_map(ind_i):
    # num_rooms = len(session.envs[0].rooms)
    num_rooms = len(rooms) + 1
    # action = session.replay_buffer.episode_data.action[ind[ind_i], :]
    action = episode_data_action[ind_i, :]
    step_indices = torch.tensor([num_rooms])
    room_mask, room_position_x, room_position_y = reconstruct_room_data(action, step_indices, num_rooms)

    doors_dict = {}
    doors_cnt = {}
    door_pairs = []
    toilet_intersections = []
    toilet_y = int(room_position_y[0, toilet_idx])
    toilet_x = int(room_position_x[0, toilet_idx])
    for i, room in enumerate(rooms):
        room_width = len(room.map[0])
        room_height = len(room.map)
        room_x = int(room_position_x[0, i])
        room_y = int(room_position_y[0, i])
        if i != toilet_idx:
            rel_x = toilet_x - room_x
            rel_y = toilet_y - room_y
            if 0 <= rel_x < room_width:
                for y in range(rel_y + 2, rel_y + 8):
                    if 0 <= y < room_height and room.map[y][rel_x] == 1:
                        toilet_intersections.append(i)
                        break
        for door in room.door_ids:
            x = room_x + door.x
            if door.direction == Direction.RIGHT:
                x += 1
            y = room_y + door.y
            if door.direction == Direction.DOWN:
                y += 1
            vertical = door.direction in (Direction.DOWN, Direction.UP)
            key = (x, y, vertical)
            if key in doors_dict:
                a = doors_dict[key]
                b = door
                if a.direction in (Direction.LEFT, Direction.UP):
                    a, b = b, a
                if a.subtype == DoorSubtype.SAND:
                    door_pairs.append([[a.exit_ptr, a.entrance_ptr], [b.exit_ptr, b.entrance_ptr], False])
                else:
                    door_pairs.append([[a.exit_ptr, a.entrance_ptr], [b.exit_ptr, b.entrance_ptr], True])
                doors_cnt[key] += 1
            else:
                doors_dict[key] = door
                doors_cnt[key] = 1

    assert all(x == 2 for x in doors_cnt.values())
    assert len(toilet_intersections) == 1
    map = {
        'rooms': [[room_position_x[0, i].item(), room_position_y[0, i].item()]
                  for i in range(room_position_x.shape[1] - 1)],
        'doors': door_pairs,
        'toilet_intersections': toilet_intersections,
    }
    return map


for room in rooms:
    room.populate()

for ind_i in range(start_index, end_index):
    logging.info("ind_i={} ({}-{})".format(ind_i, start_index, end_index))
    map = get_map(ind_i - start_index)

    xs_min = np.array([p[0] for p in map['rooms']])
    ys_min = np.array([p[1] for p in map['rooms']])
    xs_max = np.array([p[0] + rooms[i].width for i, p in enumerate(map['rooms'])])
    ys_max = np.array([p[1] + rooms[i].height for i, p in enumerate(map['rooms'])])

    door_room_dict = {}
    for i, room in enumerate(rooms):
        for door in room.door_ids:
            door_pair = (door.exit_ptr, door.entrance_ptr)
            door_room_dict[door_pair] = i
    edges_list = []
    for conn in map['doors']:
        src_room_id = door_room_dict[tuple(conn[0])]
        dst_room_id = door_room_dict[tuple(conn[1])]
        edges_list.append((src_room_id, dst_room_id))
    for i in map['toilet_intersections']:
        edges_list.append((toilet_idx, i))

    room_graph = graph_tool.Graph(directed=True)
    for (src, dst) in edges_list:
        room_graph.add_edge(src, dst)
        room_graph.add_edge(dst, src)

    def check_connected(vertices, edges):
        vmap = {v: i for i, v in enumerate(vertices)}
        filtered_edges = [(vmap[src], vmap[dst]) for (src, dst) in edges if src in vmap and dst in vmap]
        subgraph = graph_tool.Graph(directed=False)
        for (src, dst) in filtered_edges:
            subgraph.add_edge(src, dst)
        comp, hist = graph_tool.topology.label_components(subgraph)
        num_connected_components = hist.shape[0]
        return num_connected_components == 1

    # Try to assign new areas to rooms in a way that makes areas as clustered as possible
    best_entropy = float('inf')
    best_state = None
    num_areas = 6
    for i in range(0, 2000):
        graph_tool.seed_rng(i)
        state = graph_tool.inference.minimize_blockmodel_dl(room_graph,
                                                            multilevel_mcmc_args={"B_min": num_areas, "B_max": num_areas})
        e = state.entropy()
        if e < best_entropy:
            u, block_id = np.unique(state.get_blocks().get_array(), return_inverse=True)
            assert len(u) == num_areas

            invalid = False
            # ensure that Toilet gets assigned to same area as its intersecting room
            for i in map['toilet_intersections']:
                if block_id[i] != block_id[toilet_idx]:
                    invalid = True

            # check each area to ensure it is connected and fits within the size constraints:
            for j in range(num_areas):
                indj = np.where(block_id == j)[0]
                x_range = np.max(xs_max[indj]) - np.min(xs_min[indj])
                y_range = np.max(ys_max[indj]) - np.min(ys_min[indj])
                if x_range > 58 or y_range > 28:
                    invalid = True
                if not check_connected(indj, edges_list):
                    invalid = True
            if not invalid:
                print(i, e, best_entropy)
                best_entropy = e
                best_state = state
                break

    if best_state is None:
        continue
    state = best_state

    _, area_arr = np.unique(state.get_blocks().get_array(), return_inverse=True)

    # Ensure that Landing Site is in Crateria:
    area_arr = (area_arr - area_arr[1] + num_areas) % num_areas
    logging.info("Successful area assignment")

    map['area'] = area_arr.tolist()
    filename = f'maps/{session_name}-{pool}/{ind_i}.json'
    json.dump(map, open(filename, 'w'))
