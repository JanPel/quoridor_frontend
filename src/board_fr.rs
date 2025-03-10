use dioxus::prelude::*;
use log::info;

use quoridor::*;

use crate::calc_worker::*;

const DIMENSION: usize = 9;


#[derive(Clone, Copy)]
enum HoverState {
    VerticalWall(usize, usize),
    HorizontalWall(usize, usize),
    Pawn(usize, usize),
}

impl HoverState {
    // Here we decide if this hover state is active or not for this row and column, remember that walls have are lenght 2 (so 3 squares)
    fn is_hover(&self, row: usize, col: usize) -> bool {
        match self {
            HoverState::VerticalWall(r, c) => {
                *c == col && (*r == row || *r + 1 == row || *r + 2 == row)
            }
            HoverState::HorizontalWall(r, c) => {
                *r == row && (*c == col || *c + 1 == col || *c + 2 == col)
            }
            HoverState::Pawn(r, c) => *r == row && *c == col,
        }
    }
}


fn part_of_walls(
    square_type: SquareType,
    row: usize,
    col: usize,
) -> Vec<(WallDirection, Position)> {
    let row = row as i8;
    let col = col as i8;
    let mut walls = vec![];
    match square_type {
        SquareType::VerticalBorder => {
            if col / 2 >= 8 {
                return vec![];
            }
            if row / 2 >= 1 {
                walls.push((
                    WallDirection::Vertical,
                    Position {
                        row: row / 2 - 1,
                        col: col / 2,
                    },
                ));
            };
            if row / 2 < 8 {
                walls.push((
                    WallDirection::Vertical,
                    Position {
                        row: row / 2,
                        col: col / 2,
                    },
                ));
            };
        }
        SquareType::HorizontalBorder => {
            if row / 2 >= 8 {
                return vec![];
            }
            if col / 2 >= 1 {
                walls.push((
                    WallDirection::Horizontal,
                    Position {
                        row: row / 2,
                        col: col / 2 - 1,
                    },
                ));
            }
            if col / 2 < 8 {
                walls.push((
                    WallDirection::Horizontal,
                    Position {
                        row: row / 2,
                        col: col / 2,
                    },
                ));
            };
        }
        SquareType::Corner => {
            walls.push((
                WallDirection::Horizontal,
                Position {
                    row: row / 2,
                    col: col / 2,
                },
            ));
            walls.push((
                WallDirection::Vertical,
                Position {
                    row: row / 2,
                    col: col / 2,
                },
            ));
        }
        _ => (),
    };
    walls
}

fn is_part_of_wall(board: &Board, square_type: SquareType, row: usize, col: usize) -> bool {
    let walls = part_of_walls(square_type, row, col);
    for wall in walls {
        if board.walls.is_allowed(wall.0, wall.1) {
            return true;
        }
    }
    false
}




#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum SquareType {
    Square,
    VerticalBorder,
    HorizontalBorder,
    Corner,
}

impl SquareType {
    fn width(&self) -> &'static str {
        match self {
            SquareType::Square => "w-16",
            SquareType::HorizontalBorder => "w-16",
            _ => "w-4",
        }
    }
    fn height(&self) -> &'static str {
        match self {
            SquareType::Square => "h-16",
            SquareType::VerticalBorder => "h-16",
            _ => "h-4",
        }
    }

    fn hover_state(&self, row: usize, col: usize) -> Option<HoverState> {
        if self == &SquareType::VerticalBorder && (row / 2 >= DIMENSION - 1) {
            return None;
        }
        if self == &SquareType::HorizontalBorder && (col / 2 >= DIMENSION - 1) {
            return None;
        }

        match self {
            SquareType::Square => Some(HoverState::Pawn(row, col)),
            SquareType::VerticalBorder => Some(HoverState::VerticalWall(row, col)),
            SquareType::HorizontalBorder => Some(HoverState::HorizontalWall(row, col)),
            _ => None,
        }
    }
}


pub fn QuoridorBoard(cx: Scope) -> Element {
    let rows = DIMENSION * 2 - 1;
    let cols = DIMENSION * 2 - 1;

    let hover_state: &UseState<Option<HoverState>> = use_state(&cx, || None);
    let ai_suggest_move: &UseState<Option<(Move, (usize, usize))>> = use_state(&cx, || None);
    let board_flipped = use_state(&cx, || false);

    let (worker, calc_update, board, ai_player) = use_webworker(cx);
    let progress = match &calc_update.get() {
        CalculateUpdate::Progress(progress) => (progress * 100.0).round() ,
        CalculateUpdate::Finish(_) => 0.0,
    };



    let rows: Vec<usize>= match board_flipped.get() {
        true => (0..rows).rev().collect(),
        false => (0..rows).collect(),
    };
    let encoding: &UseState<String> = use_state(&cx, || "".to_string());



    let current_ai_player = *ai_player.get();
    let players_turn = current_ai_player.is_some() && board.read().board.turn % 2 != current_ai_player.unwrap();
    let hover_square = hover_state.get().clone();
    cx.render(rsx! {
        div { class: "flex justify-center items-start space-x-4",
        div { class: "flex flex-col items-center",
            div {
                class: "board bg-gray-100 p-4 grid grid-cols-{cols}",
                rows.clone().into_iter().map(|row| {
                    rsx! {
                        div {
                            class: "flex",
                            (0..cols).map(|col| {
                                let square_type = match (row%2 == 0, col%2 ==0 ) {
                                    (true,true) => SquareType::Square,
                                    (true,false) => SquareType::VerticalBorder,
                                    (false,true) => SquareType::HorizontalBorder,
                                    (false,false) => SquareType::Corner,
                                };

                                let mut color = if col % 2 == 0 && row % 2 == 0 { "bg-amber-600" } else { "bg-amber-400" };
                                //if board.read().board.is_wall_probable_front_end(square_type, row,col) {
                                //        color = "bg-gray-500";
                                //}

                                if let Some(hover_state) = hover_state.get() {
                                    if square_type != SquareType::Square {
                                        if hover_state.is_hover(row, col) {
                                            color = "bg-amber-700";
                                        }
                                    }
                                }
                                if is_part_of_wall(&board.read().board,square_type, row,col) {
                                        color = "bg-amber-800";
                                }
                                //if let Some((Move::Wall(dir, loc), (_, _))) = ai_suggest_move.get() {
                                //        for wall in Walls::part_of_walls(square_type, row, col) {
                                //            if wall ==  (*dir, *loc) {
                                //                color = "bg-green-500";
                                //            }
                                //        }
                                //}



                                let current_hover_state = square_type.hover_state(row,col);
                                rsx!{
                                    div {
                                        class: "border-0 border-grey-300 {square_type.width()} {square_type.height()} {color} flex justify-center items-center",
                                        onmouseenter: move |_| {
                                            hover_state.set(square_type.hover_state(row, col));
                                        },
                                        onclick: move |_| {
                                            if players_turn {
                                                if let Some(hover_state) = current_hover_state {
                                                    match hover_state {
                                                        HoverState::VerticalWall(r,c) => {
                                                            let game_move = Move::Wall(WallDirection::Vertical,Position {row:(r/2) as i8,col:(c/2) as i8});
                                                            board.with_mut(|board| board.game_move(game_move));
                                                            worker.send_command(UserCommand::GameMove(game_move));
                                                            ai_suggest_move.set(None);
                                                        }
                                                        HoverState::HorizontalWall(r,c) => {
                                                            let game_move =Move::Wall(WallDirection::Horizontal,Position {row:(r/2) as i8,col:(c/2)as i8});
                                                            board.with_mut(|board| board.game_move(game_move));
                                                            worker.send_command(UserCommand::GameMove(game_move));
                                                            // Only send this if valid move.
                                                            ai_suggest_move.set(None);

                                                        }
                                                        HoverState::Pawn(_r,_c) => {
                                                            ()
                                                        }
                                                    }
                                                }
                                            }

                                        },
                                        if square_type == SquareType::Square {
                                            if let Some(pawn_index) = board.read().board.is_pawn(row/2,col/2) {
                                                    let pawn_color = if pawn_index == 0 {
                                                        "bg-slate-100"
                                                    } else {
                                                        "bg-slate-900"
                                                    };
                                                    rsx! {div {
                                                        class: "{square_type.width()} {square_type.height()} {pawn_color} rounded-full",
                                                    }
                                                }
                                            } else if let Some(pawn_move) = board.read().board.is_possible_next_pawn_location(row/2,col/2) {
                                                if let Some(hover_square) = hover_square {
                                                    if hover_square.is_hover(row, col) {
                                                        let hover_color = if board.read().board.turn % 2 == 0 {
                                                            "bg-slate-200"
                                                        } else {
                                                            "bg-slate-800"
                                                        };
                                                        rsx! {div {
                                                            class: "{square_type.width()} {square_type.height()} {hover_color} rounded-full",
                                                            onclick: move |_| { 
                                                                if players_turn {
                                                                    board.with_mut(|board| {
                                                                            let game_move = Move::PawnMove(pawn_move.0, pawn_move.1);
                                                                            let res = board.game_move(game_move);
                                                                            worker.send_command(UserCommand::GameMove(game_move));
                                                                            ai_suggest_move.set(None);
                                                                            info!("Player move result: {:?}", res);
                                                                    });
                                                                }
                                                            },
                                                        }
                                                    } 
                                                } else {
                                                    rsx! {div {}}
                                                }
                                            } else {
                                                rsx! {div {}}
                                            }
                                            } else {
                                                rsx! {div {}}
                                            }
                                        }
                                        // Add your pawn and wall rendering logic here
                                    }
                                }
                            })
                        }
                    }
                })
            },
            cx.render(rsx! {
                div { class: "flex flex-wrap justify-center items-center space-x-2 p-4",
                    div { class: "flex flex-col items-center p-2",
                        div { class: "text-3xl font-bold", "TURN" },
                        div { class: "text-4xl font-bold", "{board.read().board.turn + 1}" }
                    },
                    div { class: "flex flex-col items-center p-2",
                        div { class: "text-3xl font-bold", "WHITE" },
                        // Assuming pawn 0's walls are correctly retrieved with a direct method or similar access
                        div { class: "text-4xl font-bold", "{board.read().board.pawns[0].number_of_walls_left}" }
                    },
                    div { class: "flex flex-col items-center p-2",
                        div { class: "text-3xl font-bold", "BLACK" },
                        // Corrected to use the specific field for pawn 1 as indicated
                        div { class: "text-4xl font-bold", "{board.read().board.pawns[1].number_of_walls_left}" }
                    }
                    div { class: "w-full p-4 flex flex-col items-center",
                        div { class: "text-2xl font-semibold", "Moves History: " },
                        div { class: "w-full max-h-[200px] overflow-auto p-2",
                            div { class: "text-xl", "{board.read().historic_moves()}" }
                        }
                    }
                }
            }),
        },
        div { class: "flex flex-col space-y-2",
            button {
                class: "bg-amber-500 hover:bg-amber-700 text-white font-bold py-2 px-4 rounded",
                onclick: move |_| {
                    board_flipped.with_mut(|board| {
                        *board = !*board;
                    });
                },
                "FLIP BOARD"
            },
            div {
                class: "bg-amber-500 hover:bg-amber-700 text-white font-bold py-2 px-4 rounded",
                // Assuming 'progress' is a state or prop you're tracking
                "{progress}%"
            }
            if ai_player.get().is_none() {
                rsx!{
                button {
                    class: "bg-amber-500 hover:bg-amber-700 text-white font-bold py-2 px-4 rounded",
                    onclick: move |_| {
                        ai_player.with_mut(|ai_pawn| {
                            *ai_pawn = Some(0);
                        });
                        worker.send_command(UserCommand::SetAIPlayer(0));
                    },
                    "PLAY BLACK"
                },
                button {
                    class: "bg-amber-500 hover:bg-amber-700 text-white font-bold py-2 px-4 rounded",
                    onclick: move |_| {
                        ai_player.with_mut(|ai_pawn| {
                            *ai_pawn = Some(1);
                        });
                        worker.send_command(UserCommand::SetAIPlayer(1));
                    },
                    "PLAY WHITE"
                },
                }
            }  else {
                rsx! {div{}}
            }
        }
        }

    })
}
