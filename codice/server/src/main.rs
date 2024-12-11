use wg_2024::network::*;
use std::collections::{HashMap, VecDeque};

pub enum ServerType
{
    TextServer,
    MediaServer,
}
trait Server 
{

    
    //It gives back the shortest path possible  BFS  complexity O(V+E) , shoutout to Montressor.
    fn bfs_shortest_path(
        graph: &HashMap<NodeId, Vec<NodeId>>,
        start: NodeId,
        goal: NodeId,
    ) -> Option<Vec<NodeId>> {
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
                return Some(path);
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

  

    // Remove a neibourg from a node in case of a crash
    fn remove_neighbor(graph: &mut HashMap<NodeId, Vec<NodeId>>, node: NodeId, neighbor: NodeId) {
        if let Some(neighbors) = graph.get_mut(&node) {
            neighbors.retain(|&n| n != neighbor);
        }
    }
    
    
    /*
    Start the flood protocol to fill up the hashmap and create the tree of the graph.
    Small remainder the hash map is composed in this way HashMap<NodeId, Vec<NodeId>> , The NodeId 
    and the list of it's neighbour 
     */
   
    fn initialization(&mut self)
    {
                
    }
    
    fn start(&mut self)
    {
        
    }
    
}





fn main() {

}