# Anime4You-dl
Downloads anime from https://www.anime4you.one/
## How to use?
### You need to install `youtube-dl` before using this cli tool
### You can also show the help with **--help**
### Specify series
To specify an anime use `--name (-n) "Anime name"` or `--id (-i) id`

In this downloader the Id is the series number of anime4you

![id](https://i.imgur.com/Yll2u31.png)

### Language
With the **--gersub (-s)** flag you download the series with japanese synchronization and german subtitles

With the **--gerdub (-d)** flag you download the series with german synchronization

### Specify episodes
With the **--episodes (-e)** option you can download specified episodes | 2,5 will download episodes 2 through 5

### File pattern
With the **--file-pattern (-p) "(%series_name)-Episode-(%episode)"** option you can specify a pattern of your choice with `(%series_name)` `(%episode)`

The file extension will be recognized automatically

### Output directory
You could specify an output directory with **--output (-o)**
