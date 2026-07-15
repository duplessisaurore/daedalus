<h1 align="center">Daedalus</h1>

<div align="center">
🪽☀️🪶🌊🏛️
</div>
<div align="center">
  <strong>A micro-bootloader</strong>
</div>
<div align="center">
  A services-oriented micro-bootloader built on the Lepton3 virtual machine. 
</div>

## 🌌 Table of Contents
- [<code>🪽 Capabilities</code>](#capabilities)
- [<code>🧾 License</code>](#license)
- [<code>🎓 Acknowledgments</code>](#acknowledgements)

<a name="capabilities"></a>
## 🪽 Capabilities

There are a multitude of capabilities provided by `Daedalus` to the `Lepton3` VM for programs to communicate, and access hardware/memory.

The first and one of the most important factors is that there is no pre-emption of programs in `Daedalus`, each program must be cooporative with eachother. Each program rests in one of two states: `Running` and `Blocked`. We will explore each one later as we go through the syscalls.

The three fundamental syscalls in `Daedalus` are:

### Call

This will yield the current program, giving up its execution time and placing it in the `Blocked` state. This essentially allows one program to communicate with another by *calling* one of it's provided services under it's string name.

The format of 

<a name="license"></a>
## 🧾 License

This repository and all elements of Daedalus are licensed under AGPLv3. See the `LICENSE` file in the repository root.

Daedalus will *always* be free and open-source.

<a name="acknowledgements"></a>
## 🎓 Acknowledgments

- Thanks to ``U-Boot`` for inspiration.
- Thank you for reading this README/Learning about Daedalus! 💛
- [No generative AI will ever be used for contributions, see the AI Policy section.](./CONTRIBUTING.md)

<br>

-------------

[**Created by all Contributors to Daedalus**](https://github.com/duplessisaurore/Daedalus/graphs/contributors?all=1)

Love for everyone 💛 