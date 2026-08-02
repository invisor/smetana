import { Compartment } from '@codemirror/state'

/* Отсеки — это ключи, а не данные: значение каждого живёт в EditorState.
   Поэтому они одни на все экземпляры редактора. Будь они внутри компонента,
   состояние, сохранённое прошлым экземпляром, несло бы чужие ключи, и
   reconfigure по ним молча не сработал бы. */
export const readOnlyState = new Compartment()
export const languageState = new Compartment()
