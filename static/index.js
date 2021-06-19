(async () => {
    const START_POS = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
    const boardConfig = {
        pieceTheme: "pieces/{piece}.svg",
        draggable: true,
        orientation: "white",
        position: START_POS,
        onDragStart: () => false
    };
    const board = Chessboard("board", boardConfig);
    addEventListener("resize", () => board.resize());
    const boardElement = document.getElementById("board");
    const promotions = document.getElementById("promotions");
    const gameHistory = document.getElementById("game-history");
    const newGame = document.getElementById("new-game");
    const p1Eval = document.getElementById("p1-eval");
    const p2Eval = document.getElementById("p2-eval");
    const newGamePanel = document.getElementById("new-game-panel");
    const moveSound = new Audio("./sounds/move.ogg");
    const captureSound = new Audio("./sounds/capture.ogg");

    while (true) {
        const players = {
            "w": engineMove,
            "b": engineMove
        };
        await new Promise(resolve => {
            newGamePanel.style.visibility = "visible";
            for (const newGameButton of newGamePanel.children) {
                const mode = newGameButton.getAttribute("data-mode");
                newGameButton.onclick = () => {
                    if (mode !== "auto") {
                        board.orientation(mode);
                        players[mode[0]] = playerMove;
                    } else {
                        board.orientation("white");
                    }
                    newGamePanel.style.visibility = "hidden";
                    resolve();
                };
            }
        });

        const game = new Chess(START_POS);
        function playerMove(color) {
            return new Promise(resolve => {
                boardConfig.onDragStart = (_, piece) => piece[0] === color;
                boardConfig.onDrop = (from, to) => {
                    const move = game
                        .moves({ square: from, verbose: true })
                        .find(m => m.to === to)
                    if (move === undefined) {
                        return "snapback";
                    }
                    if (move.promotion !== undefined) {
                        promotions.style.visibility = "visible";
                        for (const promotion of promotions.children) {
                            const piece = promotion.getAttribute("data-piece");
                            promotion.style.background = `url("pieces/${game.turn()}${piece}.svg")`;
                            promotion.onclick = () => {
                                resolve({ from, to, promotion: piece.toLowerCase() });
                            };
                        }
                    } else {
                        resolve({ from, to });
                    }
                };
            });
        }
        function engineMove(color) {
            return new Promise(resolve => {
                let prevResult = null;
                engine.onmessage = result => {
                    if (result.data === null) {
                        resolve(prevResult.mv);
                        engine.onmessage = null;
                    } else {
                        prevResult = result.data;
                            const eval = board.orientation()[0] === color
                            ? p1Eval
                            : p2Eval;
                        eval.innerText = [
                            "Eval:  " + prevResult.value,
                            "Depth: " + prevResult.depth,
                            "Nodes: " + prevResult.nodes,
                            "Time:  " + prevResult.time.toFixed(2) + "s",
                            "NPS:   " + (prevResult.nodes / prevResult.time).toFixed(2)
                        ].join("\n");
                        const from = prevResult.mv.slice(0, 2);
                        const to = prevResult.mv.slice(2, 4);
                        for (const square of [...boardElement.getElementsByClassName("highlighted")]) {
                            square.classList.remove("highlighted");
                        }
                        for (const square of [from, to]) {
                            const squareElement = boardElement
                                .getElementsByClassName("square-" + square)[0];
                            squareElement.classList.add("highlighted");
                        }
                    }
                };
                engine.postMessage({
                    time_left: 75_000,
                    init_pos: START_POS,
                    moves: game.history({ verbose: true }).map(m => m.from + m.to + (m.promotion ?? ""))
                });
            });
        }

        const engine = new Worker("./lunatic.js");
        await new Promise(r => engine.onmessage = r);
        boardConfig.onMoveEnd = null;
        engine.onmessage = null;
        board.position(game.fen());
        gameHistory.innerText = game.pgn();
        p1Eval.innerText = "";
        p2Eval.innerText = "";
        gameLoop: while (!game.game_over()) {
            const event = await Promise.race([
                players[game.turn()](game.turn()).then(move => ({ event: "move", move })),
                new Promise(r => newGame.onclick = () => r({ event: "newGame" }))
            ]);
            boardConfig.onDragStart = () => false;
            boardConfig.onDrop = null;
            promotions.style.visibility = "hidden";
            for (const square of [...boardElement.getElementsByClassName("highlighted")]) {
                square.classList.remove("highlighted");
            }
            
            switch (event.event) {
                case "move":
                    const moveFlags = game.move(event.move, { sloppy: true }).flags;
                    boardConfig.onMoveEnd = () => {
                        (moveFlags.includes("c") ? captureSound : moveSound).play();
                    };
                    boardConfig.onSnapEnd = boardConfig.onMoveEnd;

                    board.position(game.fen());
                    gameHistory.innerText = game.pgn();
                    gameHistory.scrollTop = gameHistory.scrollHeight;
                    break;
                case "newGame":
                    break gameLoop;
            }
        }
        engine.terminate();
        await new Promise(r => newGame.onclick = r);
    }
})();
