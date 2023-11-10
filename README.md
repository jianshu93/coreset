# coreset

Some exploration of median [$\alpha$,$\beta$] approximation in **metric spaces** and coreset construction based on the following papers:

1. Facility Location in sublinear time.   
       Badoiu, Czumaj, Indyk, Sohler ICALP 2005
       [indyk](https://people.csail.mit.edu/indyk/fl.pdf)

2. Streaming k-means on well clustered data.  
        Braverman, Meyerson, Ostrovski, Roytman ACM-SIAM 2011 
        [braverman-1](https://web.cs.ucla.edu/~rafail/PUBLIC/116.pdf) or
        [braverman-2](https://dl.acm.org/doi/10.5555/2133036.2133039)

3. New Fraweworks for Offline and Streaming Coreset Constructions.   
        Braverman, Feldman, Lang, Statsman 2022
        [arxiv-v3](https://arxiv.org/abs/1612.00889)


## Examples

Examples provided are the standard Mnist-digits and Mnist-fashion.

## Results

### Streaming k-means on well clustered data

#### running in one pass. (~ 130 facilities)

#### running with post contraction of number of facilities. (~ 70 facilities)

This algorithm runs on both Mnist data in 0.5 second on a i9 laptop.
The mean entropy of labels distributions (10 labels) in each cluster found is between 0.82 for the digits example and 0.84 for the fashion example.

The cost is the mean of L2 distance of each of the 70000 images to its nearest facility. The distance is normalized by the number of pixels (coded in the range [0..256])

|  mnist       |  mean entropy  |    cost      |  nb facility | 
|  :---:       |  :---:         |    :---:     |     :---:    |
|   digits     |    0.82        |     2.13     |      69      |
|   fashion    |    0.837       |     1.78     |      75      |
    

## License

Licensed under either of

* Apache License, Version 2.0, [LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>
* MIT license [LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>

at your option.