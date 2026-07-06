rAthena-MapServerOnRust
High-Performance Map Server Accelerator for rAthena.

Breaking the single-threaded bottleneck with Rust and Shared Memory.

🚀 The Vision
rAthena is a legendary project, but it is limited by its single-threaded C++ architecture. As player density increases (e.g., 500-1,000 players in a single Guild War), the main core struggles with CPU spikes.

This project aims to revolutionize RO server performance. We are offloading heavy-duty game logic—such as Pathfinding, AoE Calculations, and Spatial Partitioning—to a high-performance Rust engine, turning the rAthena core into a lean, fast data gateway.

🛠 Why Rust & Shared Memory?
Zero-Copy Communication: We use Shared Memory for instant, low-latency data exchange between C++ and Rust.

Parallel Computing: Rust handles complex calculations across multiple CPU cores, leaving the rAthena core free to manage network I/O.

Safety & Performance: Leveraging Rust's memory safety to prevent crashes while achieving near-native performance.

⚙️ Current Capabilities
Shared Memory Bridge: Established communication between rAthena (C++) and Rust.

Movement Processing: Successfully offloaded player movement logic to the Rust engine.

Architecture Ready: Designed to work as a "Logic Offloader" sidecar to the rAthena core.

🤝 Call for Collaborators: We Need You!
This is a high-level engineering challenge. We are looking for talented developers who want to push the boundaries of MMORPG server technology. If you have expertise in:

Low-level Systems Programming: Experience with C++ and Rust FFI.

Memory Management: Knowledge of Shared Memory, Ring Buffers, and Atomic Operations.

Algorithm Optimization: Expertise in A* Pathfinding, Spatial Partitioning (Grids/Quadtrees), or Data-Oriented Design.

Game Engine Architecture: Understanding of server-side game loops and network synchronization.

Join us to build the most efficient Ragnarok server engine in existence!

🏗 Roadmap
[ ] Implement Combat/Skill interaction (The next big milestone).

[ ] Optimize Spatial Indexing to support 1,000+ entities.

[ ] Lock-free synchronization for high-frequency updates.
