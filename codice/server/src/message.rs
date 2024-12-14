#![allow(warnings)]

/*
This module gives the essential function that allow the server and the client to interact with the network
The principal function are :
    bfs_shortest_path (Tree graph , start_ID , goal_ID)-> gives back the shortest path from start to goal
    remove_neighbor (Tree graph , node_ID, neighbour_ID ) -> remove a neighbour from a node




Guide for implementing the servers and the clients

    First of all you have to create the network graph so you send a FloodRequest to each node DIRECTLY  conneted to the server/client
    here is an example on how the function should be


*/

pub mod net_work {

    use std::collections::{HashMap, VecDeque};
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::NodeType;

    //It gives back the shortest path possible  BFS  complexity O(V+E) , shutout to Montresor.
    pub fn bfs_shortest_path(
        graph: &HashMap<NodeId, Vec<NodeId>>,
        start: NodeId,
        goal: NodeId,
    ) -> Option<SourceRoutingHeader> {
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();
        let mut parent = HashMap::new();

        queue.push_back(start);
        visited.insert(start, true);

        while let Some(current) = queue.pop_front() {
            if current == goal {
                // Reconstruct of the path
                let mut path = vec![goal];
                let mut node = goal;
                while let Some(&p) = parent.get(&node) {
                    path.push(p);
                    node = p;
                }
                path.reverse();
                return Some(SourceRoutingHeader {
                    hop_index: 0,
                    hops: path,
                });
            }

            if let Some(neighbors) = graph.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains_key(&neighbor) {
                        queue.push_back(neighbor);
                        visited.insert(neighbor, true);
                        parent.insert(neighbor, current);
                    }
                }
            }
        }

        None // No path found !BIG PROBLEM!
    }

    // Remove a neighbour from a node in case of a crash
    pub fn remove_neighbor(
        graph: &mut HashMap<NodeId, Vec<NodeId>>,
        node: NodeId,
        neighbor: NodeId,
    ) {
        if let Some(neighbors) = graph.get_mut(&node) {
            neighbors.retain(|&n| n != neighbor);
        }
    }

    //this function start creating the graph with the flood response that it receive
    pub fn recive_flood_response(graph: &mut HashMap<NodeId, Vec<NodeId>>, lead: Vec<(NodeId,NodeType)>) {

        let path: Vec<NodeId> = lead
            .iter()
            .map(|(node_id, _)| *node_id)
            .collect();
        
        
        let mut prec: NodeId = 0;

        for (numb, &i) in path.iter().enumerate() {
            // Add the current node to the graph if it doesn't exist
            graph.entry(i).or_insert_with(Vec::new);

            // Skip the first iteration (no previous node exists)
            if numb != 0 {
                // Add `prec` as a neighbor of `i`
                if let Some(neighbors) = graph.get_mut(&i) {
                    if !neighbors.contains(&prec) {
                        neighbors.push(prec);
                    }
                }

                // Add `i` as a neighbor of `prec`
                graph.entry(prec).or_insert_with(Vec::new);
                if let Some(neighbors) = graph.get_mut(&prec) {
                    if !neighbors.contains(&i) {
                        neighbors.push(i);
                    }
                }
            }

            // Update `prec` for the next iteration
            prec = i;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use net_work::*;
    use std::collections::HashMap;
    use wg_2024::network::NodeId;
    use wg_2024::packet::NodeType::Drone;

    #[test]
    fn test_bfs_shortest_path() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        graph.insert(1, vec![2, 3]);
        graph.insert(2, vec![1, 4]);
        graph.insert(3, vec![1]);
        graph.insert(4, vec![2]);

        let start = 1;
        let goal = 4;

        let path = bfs_shortest_path(&graph, start, goal);
        if let Some(header) = path {
            assert_eq!(header.hops, vec![1, 2,4]);
        }
    }

    #[test]
    fn test_remove_neighbor() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        graph.insert(1, vec![2, 3]);
        graph.insert(2, vec![1, 4]);
        graph.insert(3, vec![1]);
        graph.insert(4, vec![2]);

        remove_neighbor(&mut graph, 1, 2);
        assert_eq!(graph.get(&1), Some(&vec![3])); // Neighbor 2 should be removed from node 1

        remove_neighbor(&mut graph, 4, 2);
        assert_eq!(graph.get(&4), Some(&vec![])); // Neighbor 2 should be removed from node 4
    }

    #[test]
    fn test_receive_flood_response() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let path = vec![(1,Drone),(2,Drone),(3,Drone),(4,Drone)]; // Simulated path from a flood response

        recive_flood_response(&mut graph, path);

        // Check if the graph structure is correctly updated
        assert_eq!(graph.get(&1), Some(&vec![2]));
        assert_eq!(graph.get(&2), Some(&vec![1, 3]));
        assert_eq!(graph.get(&3), Some(&vec![2, 4]));
        assert_eq!(graph.get(&4), Some(&vec![3]));
    }

    #[test]
    fn test_complex_flood_response() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        // Simulate multiple flood responses
        recive_flood_response(&mut graph, vec![(1,Drone),(2,Drone),(3,Drone)]);
        recive_flood_response(&mut graph, vec![(3,Drone),(4,Drone),(5,Drone)]);
        recive_flood_response(&mut graph, vec![(5,Drone),(6,Drone)]);

        // Check the resulting graph
        assert_eq!(graph.get(&1), Some(&vec![2]));
        assert_eq!(graph.get(&2), Some(&vec![1, 3]));
        assert_eq!(graph.get(&3), Some(&vec![2, 4]));
        assert_eq!(graph.get(&4), Some(&vec![3, 5]));
        assert_eq!(graph.get(&5), Some(&vec![4, 6]));
        assert_eq!(graph.get(&6), Some(&vec![5]));
    }
}
