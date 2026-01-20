class Person:
    def __init__(self, age=0):
        self._age = age

    def set_age(self, new_age):
        if new_age < 0:
            print(
                "Дорогой друг неужели ты прожил настолько долго что твй возраст аж пошел в обратную сторону?"
            )
        self._age = new_age

    def get_age(self):
        return self._age


p = Person()
p.set_age(25)
print(p.get_age())
p.set_age(-5)


class Animal:
    def __init__(self, name):
        self.name = name

    def speak(self):
        return "I am an animal"


class Dog(Animal):
    def __init__(self, name):
        super().__init__(name)

    def speak(self):
        return "Woof"


class Cat(Animal):
    def __init__(self, name):
        super().__init__(name)

    def speak(self):
        return "Meow"


dog = Dog("Buddy")
cat = Cat("Kitty")

print(dog.name, dog.speak())  # Вывод: Buddy Woof
print(cat.name, cat.speak())  # Вывод: Kitty Meow


class Car:
    def move(self):
        return "car is driving"


class Bicycle:
    def move(self):
        return "Bycikle is driving"


def move(obj):
    return obj.move()


car = Car()
bike = Bicycle()

print(move(car))  # Вывод: Car is driving
print(move(bike))  # Вывод: Bicycle is pedaling


"""

ЧЕСТНО ТУТ Я СХАЛТУРИЛ И СКОПИПАСТИЛ ТО ЧТО ВЫДАЛА НЕЙРОНКА ИБО УЖЕ 12:25 А ЗАВТРА НА РАБОТУ 

"""
from abc import ABC, abstractmethod
import math


# Абстрактный класс
class Shape(ABC):
    @abstractmethod
    def area(self):
        """Метод для вычисления площади фигуры"""
        pass


# Наследник: Прямоугольник
class Rectangle(Shape):
    def __init__(self, width, height):
        self.width = width
        self.height = height

    def area(self):
        return self.width * self.height


# Наследник: Круг
class Circle(Shape):
    def __init__(self, radius):
        self.radius = radius

    def area(self):
        # Формула площади круга: S = pi * r^2
        return math.pi * (self.radius**2)


# Пример использования
if __name__ == "__main__":
    rect = Rectangle(10, 5)
    circle = Circle(7)

    print(f"Площадь прямоугольника: {rect.area()}")  # Вывод: 50
    print(f"Площадь круга: {circle.area():.2f}")  # Вывод: ~153.94
