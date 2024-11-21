
import torch
import logging
from maze_builder.env import MazeBuilderEnv
from maze_builder.types import reconstruct_room_data, Direction
import logic.rooms.all_rooms
# import logic.rooms.crateria_isolated
# import logic.rooms.norfair_isolated
import pickle
import concurrent.futures
import random


for i, room in enumerate(logic.rooms.all_rooms.rooms):
    print(i, room.name)


logging.basicConfig(format='%(asctime)s %(message)s',
                    level=logging.INFO,
                    handlers=[logging.FileHandler("train.log"),
                              logging.StreamHandler()])

torch.set_printoptions(linewidth=120, threshold=10000)
import io


class CPU_Unpickler(pickle.Unpickler):
    def find_class(self, module, name):
        if module == 'torch.storage' and name == '_load_from_bytes':
            return lambda b: torch.load(io.BytesIO(b), map_location='cpu')
        else:
            return super().find_class(module, name)

device = torch.device('cpu')
# session = CPU_Unpickler(open('models/07-31-session-2022-06-03T17:19:29.727911.pkl-bk30-small', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-05-10T14:34:48.668019.pkl-small.pkl', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-05-31T14:35:04.410129.pkl', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-05-31T21:25:13.243815.pkl', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-02T23:26:53.466014.pkl', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-10', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-16', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-22', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-34', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-43', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-50', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-61', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-63', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-06-08T14:55:16.779895.pkl-small-70', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-1', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-22', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-31', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-40', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-44', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-46', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-47', 'rb')).load()
# session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-48', 'rb')).load()
session = CPU_Unpickler(open('models/session-2023-11-08T16:16:55.811707.pkl-small-51', 'rb')).load()

print(torch.sort(torch.sum(session.replay_buffer.episode_data.missing_connects.to(torch.float32), dim=0)))
# min_reward = torch.min(session.replay_buffer.episode_data.reward)
# print(min_reward, torch.mean((session.replay_buffer.episode_data.reward == min_reward).to(torch.float32)),
#       session.replay_buffer.episode_data.reward.shape[0])

S = session.replay_buffer.episode_data.save_distances.to(torch.float32)
S = torch.where(S == 255, torch.full_like(S, float('nan')), S)
S = torch.nanmean(S, dim=1)
# print(torch.nanmean(S))

M = session.replay_buffer.episode_data.mc_distances.to(torch.float32)
M = torch.where(M == 255, torch.full_like(M, float('nan')), M)
M = torch.nanmean(M, dim=1)

# ind = torch.nonzero((session.replay_buffer.episode_data.reward >= 340) & (session.replay_buffer.episode_data.temperature > 0.5))
# ind = torch.nonzero((session.replay_buffer.episode_data.reward >= 343) & (session.replay_buffer.episode_data.temperature < 0.05))
# ind = torch.nonzero(session.replay_buffer.episode_data.reward >= 343)
# ind = torch.nonzero(session.replay_buffer.episode_data.reward >= 0)
# ind = ind[(ind >= 200000) & (ind < 262144)].view(-1, 1)
# num_feasible = torch.nonzero((session.replay_buffer.episode_data.reward == min_reward)).shape[0]

ind = torch.nonzero(
    (session.replay_buffer.episode_data.reward == 0) &
    (S < 4.05) &
    (session.replay_buffer.episode_data.graph_diameter <= 43) &
    # (session.replay_buffer.episode_data.mc_dist_coef > 0.0)
    (session.replay_buffer.episode_data.mc_dist_coef == 0.0) &
    session.replay_buffer.episode_data.toilet_good
)

# ind = torch.nonzero(
#     (session.replay_buffer.episode_data.reward == min_reward) #&
#     # (S < 3.90) &
#     # (session.replay_buffer.episode_data.graph_diameter <= 45) &
#     # (session.replay_buffer.episode_data.mc_dist_coef > 0.0)
# )

# print(sorted(M[ind].tolist()))
# print(sorted(torch.amax(session.replay_buffer.episode_data.mc_distances[ind], dim=1).tolist()))
# print(torch.mean(torch.amax(session.replay_buffer.episode_data.mc_distances[ind], dim=1).to(torch.float)))

# print(sorted(M[ind].tolist()))
# print(torch.where(session.replay_buffer.episode_data.graph_diameter[ind] == 29))

# print("success rate: ", ind.shape[0] / num_feasible)
i = int(random.randint(0, ind.shape[0] - 1))
# i = 26
print(len(ind), i)
# i = 389
num_rooms = len(session.envs[0].rooms)
# print("mean save_distance:", torch.mean(session.replay_buffer.episode_data.save_distances[ind].to(torch.float)))
# print("mean diam:", torch.mean(session.replay_buffer.episode_data.graph_diameter[ind].to(torch.float)))
# print("max diam:", torch.max(session.replay_buffer.episode_data.graph_diameter[ind]))
# print("min diam:", torch.min(session.replay_buffer.episode_data.graph_diameter[ind]))
# print("diam:", session.replay_buffer.episode_data.graph_diameter[ind[i]])

action = session.replay_buffer.episode_data.action[ind[i], :]
# action = session.replay_buffer.episode_data.action[ind[:16], :]
step_indices = torch.tensor([num_rooms])
room_mask, room_position_x, room_position_y = reconstruct_room_data(action, step_indices, num_rooms)


# env = session.envs[0]
# A = env.compute_part_adjacency_matrix(room_mask, room_position_x, room_position_y)
# # A = env.compute_part_adjacency_matrix(env.room_mask, env.room_position_x, env.room_position_y)
# D = env.compute_distance_matrix(A)


# env = session.envs[0]
# A = env.compute_part_adjacency_matrix(room_mask, room_position_x, room_position_y)
# A = env.compute_part_adjacency_matrix(env.room_mask, env.room_position_x, env.room_position_y)
# D = env.compute_distance_matrix(A)
# S = env.compute_save_distances(D)
# M = env.compute_missing_connections(A)
# print(torch.sum(M, dim=1))



# print(torch.where(session.replay_buffer.episode_data.missing_connects[ind[i, 0], :] == False))
# print(torch.where(room_mask[0, :251] == False))
# print(torch.where(session.replay_buffer.episode_data.door_connects[ind[i, 0], :] == False))
# dir(session.envs[0])

#
# num_envs = 2
# num_envs = 8
rooms = logic.rooms.all_rooms.rooms
# rooms = logic.rooms.crateria_isolated.rooms
# rooms = logic.rooms.norfair_isolated.rooms


# doors = {}
# for room in rooms:
#     for door in room.door_ids:
#         key = (door.exit_ptr, door.entrance_ptr)
#         doors[key] = door
# for key in doors:
#     exit_ptr, entrance_ptr = key
#     reversed_key = (entrance_ptr, exit_ptr)
#     if reversed_key not in doors:
#         print('{:x} {:x}'.format(key[0], key[1]))
#     else:
#         door = doors[key]
#         reversed_door = doors[reversed_key]
#         assert door.subtype == reversed_door.subtype
#         if door.direction == Direction.DOWN:
#             assert reversed_door.direction == Direction.UP
#         elif door.direction == Direction.UP:
#             assert reversed_door.direction == Direction.DOWN
#         elif door.direction == Direction.RIGHT:
#             assert reversed_door.direction == Direction.LEFT
#         elif door.direction == Direction.LEFT:
#             assert reversed_door.direction == Direction.RIGHT
#         else:
#             assert False


# num_envs = 4
num_envs = 1
episode_length = len(rooms)
env = MazeBuilderEnv(rooms,
                     map_x=session.envs[0].map_x,
                     map_y=session.envs[0].map_y,
                     num_envs=num_envs,
                     starting_room_name="Landing Site",
                     # starting_room_name="Business Center",
                     device=device,
                     must_areas_be_connected=False)
env.room_position_x = room_position_x
env.room_position_y = room_position_y
env.room_mask = room_mask
env.render(0)
env.map_display.image.show()

self = env
toilet_idx = self.toilet_idx
toilet_x = room_position_x[:, toilet_idx].view(-1, 1)
toilet_y = room_position_y[:, toilet_idx].view(-1, 1)
toilet_mask = room_mask[:, toilet_idx].view(-1, 1)

good_toilet_room_idx = self.good_toilet_positions[:, 0]
good_toilet_x = self.good_toilet_positions[:, 1].view(1, -1)
good_toilet_y = self.good_toilet_positions[:, 2].view(1, -1)
good_room_x = room_position_x[:, good_toilet_room_idx]
good_room_y = room_position_y[:, good_toilet_room_idx]
good_room_mask = room_mask[:, good_toilet_room_idx]
good_match = (toilet_x == good_room_x + good_toilet_x) & (
toilet_y == good_room_y + good_toilet_y) & toilet_mask & good_room_mask
num_good_match = torch.sum(good_match, dim=1)
