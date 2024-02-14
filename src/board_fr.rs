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
        CalculateUpdate::Progress(progress) => *progress,
        CalculateUpdate::Finish(_) => 0.0,
    };



    let rows: Vec<usize>= match board_flipped.get() {
        true => (0..rows).rev().collect(),
        false => (0..rows).collect(),
    };
    let encoding: &UseState<String> = use_state(&cx, || "".to_string());



    let current_ai_player = *ai_player.get();
    let players_turn = current_ai_player.is_some() && board.read().turn % 2 != current_ai_player.unwrap();
    cx.render(rsx! {
        div { class: "flex",
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

                                let mut color = if col % 2 == 0 && row % 2 == 0 { "bg-grey-200" } else { "bg-blue-500" };
                                //if board.read().board.is_wall_probable_front_end(square_type, row,col) {
                                //        color = "bg-gray-500";
                                //}

                                if let Some(hover_state) = hover_state.get() {
                                    if hover_state.is_hover(row, col) {
                                        color = "bg-green-500";
                                    }
                                }
                                if is_part_of_wall(&board.read(),square_type, row,col) {
                                        color = "bg-red-500";
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
                                            if let Some(pawn_index) = board.read().is_pawn(row/2,col/2) {
                                                    let pawn_color = if pawn_index == 0 {
                                                        "bg-yellow-500"
                                                    } else {
                                                        "bg-purple-500"
                                                    };
                                                    rsx! {div {
                                                        class: "{square_type.width()} {square_type.height()} {pawn_color} rounded-full",
                                                    }
                                                }
                                            } else if let Some(pawn_move) = board.read().is_possible_next_pawn_location(row/2,col/2) {
                                                if let Some((Move::PawnMove(first_move,second_move),(move_row ,move_col))) = ai_suggest_move.get(){
                                                    if (*move_row, *move_col) == (row/2, col/2) {
                                    rsx! {div {
                                                            class: "{square_type.width()} {square_type.height()} bg-red-500 rounded-full",
                                                            onclick: move |_| { 

                                                                if players_turn {
                                                                    board.with_mut(|board| {
                                                                        let game_move = Move::PawnMove(*first_move, *second_move);
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
                                    rsx! {div {
                                                        class: "{square_type.width()} {square_type.height()} bg-gray-500 rounded-full",
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
                                                }
                                            } else {
                                                rsx! {div {
                                                        class: "{square_type.width()} {square_type.height()} bg-gray-500 rounded-full",
                                                        onclick: move |_| { 
                                                            if players_turn {
                                                                board.with_mut(|board| {
                                                                    let game_move = Move::PawnMove(pawn_move.0, pawn_move.1);
                                                                    let res = board.game_move(game_move);
                                                                    worker.send_command(UserCommand::GameMove(game_move));
                                                                    info!("Player move result: {:?}", res);
                                                                });
                                                            }
                                                        },


                                                    }
                                                }
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
            }
        }
        button {
            class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
            onclick: move |_| {
                board_flipped.with_mut(|board| {
                    *board = !*board;
                });
            },
            "FLIP BOARD"
        }
        button {
            class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
            onclick: move |_| {
                ai_player.with_mut(|ai_pawn| {
                    *ai_pawn = Some(0);
                });
                worker.send_command(UserCommand::SetAIPlayer(0));
            },
            "PLAY BLACK"
        }
        button {
            class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded",
            onclick: move |_| {
                ai_player.with_mut(|ai_pawn| {
                    *ai_pawn = Some(1);
                });
                worker.send_command(UserCommand::SetAIPlayer(1));
            },
            "PLAY WHITE"
        }
        div {
            class: "bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded w-18",
                "{progress}%"
        }
        


        
    })
}
