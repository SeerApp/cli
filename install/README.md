Testing install script using Docker. 

For amd64, build the image: 
```
docker buildx build --platform=linux/amd64 -t test-install -f ./Dockerfile.amd64 .
```

Then, run the container:
```
docker run --rm -it --platform=linux/amd64 test-install
```