run() {
    cat $1 | ruca > main.asm
    shift

    nasm -f elf64 -g -F dwarf -o main.o main.asm
    gcc -no-pie -z noexecstack -O3 -rdynamic -o main main.o
    ./main "$@"
    echo $? > /dev/null
}

install() {
    sudo apt install nasm gcc pkg-config curl
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    . "$HOME/.cargo/env"
    cargo install --path .
}

update() {
    git stash
    git pull
    cargo install --path .
}

if  [ "$#" -ne 1 ]; then
    run "$@"
elif [ $1 = "--update" ]; then
    update
elif [ $1 = "--install" ]; then
    install
else
    run "$@"
fi
