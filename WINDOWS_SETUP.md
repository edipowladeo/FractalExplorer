# Configuração no Windows

Este guia configura o ambiente necessário para compilar e testar o projeto sem instalar o Visual Studio completo. Usaremos o Rust via `rustup` e o linker GCC fornecido pelo MSYS2/UCRT64.

## 1. Instalar o Rust

Abra o PowerShell e execute:

```powershell
winget install Rustlang.Rustup
```

O `rustup` não é redundante. O MSYS2 fornece o GCC/linker para Windows, mas o Rust, o Cargo e a toolchain Rust continuam sendo necessários para compilar o projeto.

Feche e reabra o PowerShell depois da instalação. Confirme:

```powershell
rustc --version
cargo --version
```

## 2. Instalar o MSYS2

No PowerShell:

```powershell
winget install MSYS2.MSYS2
```

Depois abra o menu Iniciar e execute **MSYS2 UCRT64**.

No terminal MSYS2 UCRT64, atualize os pacotes:

```bash
pacman -Syu
```

Se o terminal pedir para fechar e reabrir, faça isso e execute novamente:

```bash
pacman -Su
pacman -S --needed mingw-w64-ucrt-x86_64-toolchain
```

O compilador ficará normalmente em:

```text
C:\msys64\ucrt64\bin
```

## 3. Configurar a toolchain GNU do Rust

No PowerShell normal, execute:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
```

## 4. Persistir o PATH do MinGW

O comando `$env:Path += ...` altera somente a sessão atual. Para persistir o caminho no PATH do usuário, execute no PowerShell:

```powershell
$mingwPath = "C:\msys64\ucrt64\bin"
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")

if (($userPath -split ";") -notcontains $mingwPath) {
    [Environment]::SetEnvironmentVariable(
        "Path",
        "$userPath;$mingwPath",
        "User"
    )
}
```

Feche todas as janelas do PowerShell e abra uma nova. Confirme:

```powershell
$env:Path -split ";" | Select-String "msys64"
gcc --version
dlltool --version
```

Não use `setx PATH "$env:PATH;..."` para essa configuração: ele pode truncar um PATH longo.

O `dlltool.exe` é instalado pelo pacote `mingw-w64-ucrt-x86_64-binutils`, incluído no grupo `mingw-w64-ucrt-x86_64-toolchain`. Ele é necessário quando o alvo GNU do Rust compila dependências nativas da feature `native-ui`, como `eframe`, `winit` e `wgpu`.

Se o MSYS2 já estiver instalado, a instalação pode ser feita pelo PowerShell com:

```powershell
& "C:\msys64\usr\bin\pacman.exe" -S --needed mingw-w64-ucrt-x86_64-toolchain
```

## 5. Validar a UI nativa

Depois de reabrir o PowerShell para carregar o `PATH` persistido, execute:

```powershell
cd C:\Users\edipo\repo\fractalrenderers\FractalExplorer
cargo check --features native-ui
cargo run --features native-ui --bin sprite-demo
```

O segundo comando abre a janela de configuração `egui/eframe` e a janela de renderização `minifb` ao mesmo tempo. Feche a janela de configuração para deixar o renderer continuar, ou pressione `Esc` na janela do fractal para encerrá-lo.

## 6. Compilar e testar

No PowerShell novo:

```powershell
cd C:\Users\edipo\repo\fractalrenderers\FractalExplorer
cargo test
```

Para abrir o protótipo do sprite:

```powershell
cargo run --bin sprite-demo
```

Pressione `Esc` para fechar a janela.

## Diagnóstico rápido

Verifique a toolchain ativa:

```powershell
rustup show active-toolchain
```

Ela deve indicar:

```text
stable-x86_64-pc-windows-gnu
```

Se `cargo` não for reconhecido, reabra o terminal e confirme se `C:\Users\<seu-usuário>\.cargo\bin` está no PATH. Se `gcc` ou `dlltool` não forem reconhecidos, confirme se `C:\msys64\ucrt64\bin` está no PATH e repita a instalação do grupo `mingw-w64-ucrt-x86_64-toolchain`.
