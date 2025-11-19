FROM nixos/nix

RUN nix-channel --update

RUN echo 'experimental-features = nix-command flakes' >> /etc/nix/nix.conf &&\
    nix profile add nixpkgs\#vim nixpkgs\#direnv nixpkgs\#gawk nixpkgs\#gnused &&\
    sed -i '/nixbld1:/ { s:/var/empty:/homedir: }' /etc/passwd &&\
    mkdir /code &&\
    mkdir /homedir && chown -R nixbld1: /homedir /code &&\
    chown -R nixbld1: /nix &&\
    direnv hook bash > /homedir/.bashrc
    
WORKDIR /code

USER nixbld1

ADD .envrc /code

RUN direnv allow && rm .envrc
