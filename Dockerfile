FROM ubuntu:24.04


ENV PATH=/lib/llvm-19/bin:/usr/local/cargo/bin:/root/.cargo/bin:$PATH \ 
    LD_LIBRARY_PATH=/lib/llvm-19/lib:$LD_LIBRARY_PATH \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    DEBIAN_FRONTEND=noninteractive \
    DOCKER_CONTAINER=1

# update cn mirror 
RUN <<EOT bash
echo 'install ca-certificates'
apt update
apt install -y ca-certificates

echo 'dumping mirrors'
cat <<'EOF' > /etc/apt/sources.list.d/ubuntu.sources
Types: deb
URIs: https://mirrors.cqu.edu.cn/ubuntu
Suites: noble noble-updates noble-backports
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

# 默认注释了源码镜像以提高 apt update 速度，如有需要可自行取消注释
# Types: deb-src
# URIs: https://mirrors.cqu.edu.cn/ubuntu
# Suites: noble noble-updates noble-backports
# Components: main restricted universe multiverse
# Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

# 以下安全更新软件源包含了官方源与镜像站配置，如有需要可自行修改注释切换
# Types: deb
# URIs: https://mirrors.cqu.edu.cn/ubuntu
# Suites: noble-security
# Components: main restricted universe multiverse
# Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

# # Types: deb-src
# # URIs: https://mirrors.cqu.edu.cn/ubuntu
# # Suites: noble-security
# # Components: main restricted universe multiverse
# # Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

Types: deb
URIs: http://security.ubuntu.com/ubuntu/
Suites: noble-security
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

# Types: deb-src
# URIs: http://security.ubuntu.com/ubuntu/
# Suites: noble-security
# Components: main restricted universe multiverse
# Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

# 预发布软件源，不建议启用
# Types: deb
# URIs: https://mirrors.cqu.edu.cn/ubuntu
# Suites: noble-proposed
# Components: main restricted universe multiverse
# Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

# # Types: deb-src
# # URIs: https://mirrors.cqu.edu.cn/ubuntu
# # Suites: noble-proposed
# # Components: main restricted universe multiverse
# # Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
EOF

apt update

EOT

RUN apt-get update \
    && apt-get -y install build-essential ninja-build wget curl cmake git pipx unzip patchelf graphviz python3 python3-pip lsb-release software-properties-common gnupg file libssl-dev openssl pkg-config libfontconfig libfontconfig1-dev zip \
    && apt install -y lsb-release wget software-properties-common gnupg golang-go\
    && apt-get clean 

# add go PATH env var
ENV PATH=/root/go/bin:$PATH

RUN go install github.com/SRI-CSL/gllvm/cmd/...@latest

RUN pip config set global.index-url https://mirrors.aliyun.com/pypi/simple

RUN pipx install wllvm

# build llvm and clang dependency
RUN wget https://apt.llvm.org/llvm.sh \
    && chmod +x llvm.sh \
    && ./llvm.sh 19 


ENV RUSTUP_DIST_SERVER="https://rsproxy.cn" \ 
  RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"

RUN curl --proto '=https' --tlsv1.2 -sSf https://rsproxy.cn/rustup-init.sh | sh -s -- -y --default-toolchain stable && rustup default stable

RUN  mkdir -p /usr/local/cargo && cat > /usr/local/cargo/config.toml <<EOF
[source.crates-io]
replace-with = 'rsproxy-sparse'
[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"
[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
[registries.rsproxy]
index = "https://rsproxy.cn/crates.io-index"
[net]
git-fetch-with-cli = true
EOF

# install library dependencies
RUN apt install -y yasm vim tree libcurl4-openssl-dev libedit-dev libzstd-dev

RUN mkdir -p /struct_fuzz
WORKDIR /struct_fuzz
COPY . .
