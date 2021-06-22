#! /bin/bash
# https://gist.github.com/schell/2fe896953b6728cc3c5d8d5f9f3a17a3
# requires curl and jq on PATH: https://stedolan.github.io/jq/

# upload a release file.
# this must be called only after a successful create_release, as create_release saves
# the json response in release.json.
# token: github api user token
# repo: github username/reponame
# file: path to the asset file to upload
# name: name to use for the uploaded asset
upload_release_file() {
    token=$1
    repo=$2
    file=$3
    name=$4

    url=`curl --silent "https://api.github.com/repos/${repo}/releases/latest" | jq -r .upload_url | cut -d{ -f'1'`
    command="\
      curl -s -o upload.json -w '%{http_code}' \
           --request POST \
           --header 'authorization: Bearer ${token}' \
           --header 'Content-Type: application/octet-stream' \
           --data-binary @\"${file}\"
           ${url}?name=${name}"
    http_code=`eval $command`
    if [ $http_code == "201" ]; then
        echo "asset $name uploaded:"
        jq -r .browser_download_url upload.json
    else
        echo "upload failed with code '$http_code':"
        cat upload.json
        echo "command:"
        echo $command
        return 1
    fi
}
