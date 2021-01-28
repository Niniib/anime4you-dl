As of 2021/01/26, anime4you decided to stop their project and to join serienstream (s.to). If you want to continue downloading animes/series, [a contributor from this project also made a somewhat functional downloader for serienstream.](https://github.com/Fludixx/serienstream-dl)

# Anime4You-dl
Downloads anime from https://www.anime4you.one/
## How to use?
To faster download episodes you can use the prebuild `db.bin` which is located in this repository. Place it in the same directory as the executable.
### You can also show the help with **--help**
### Specify series
To specify an anime use `--name (-n) "Anime name"` or `--id (-i) id`

The Id is the series number of anime4you

![id](https://i.imgur.com/Yll2u31.png)

### Language
With the **--gersub (-s)** flag you download the series with japanese dubbing and german subtitles

With the **--gerdub (-d)** flag you download the series with german dubbing

### Specify episodes
With the **--episodes (-e)** option you can download specified episodes | 2,5 will download episodes 2 through 5

### File pattern
With the **--file-pattern (-p) "(%series_name)-Episode-(%episode)"** option you can specify a pattern of your choice with `(%series_name)` `(%episode)`

The file extension will be added automatically

### Parallel downloads

If you have a fast internet connection you can add **--parallel** to download multiple episodes at once.

### youtube-dl
With the **--youtube-dl (-y)** flag you download the series with `youtube-dl`

### Output directory
You could specify an output directory with **--output (-o)**
